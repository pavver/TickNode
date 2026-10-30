#!/usr/bin/env bash
# Rebuilds TickNode and (re)starts the server for local development; reload the browser afterwards.
#
#   ./dev.sh                  build plugins, editor and server, then restart the server
#   ./dev.sh editor server    build only the listed parts (plugins | editor | server), then restart
#
# The server runs against `.dev-data/` (a copy of `data/graph.json` on the first run), never the real
# `data/` directory. Release plugins are taken from `target/` through TICKNODE_BUNDLED_PLUGINS_DIR.
# Running the script again stops the previous server (also when it runs in another terminal).
set -euo pipefail

cd "$(dirname "$0")"
ROOT=$PWD
DATA=$ROOT/.dev-data
BUNDLED=$DATA/bundled-plugins
PIDFILE=$DATA/server.pid
PLUGINS=(delay-plugin string-plugin math-plugin http-weather-plugin ha-plugin)

parts=("$@")
[ ${#parts[@]} -eq 0 ] && parts=(plugins editor server)
wants() { [[ " ${parts[*]} " == *" $1 "* ]]; }

mkdir -p "$DATA" "$BUNDLED"
[ -f "$DATA/graph.json" ] || { [ -f data/graph.json ] && cp data/graph.json "$DATA/"; }

if wants plugins; then
    echo "==> plugins"
    cargo build --release --target wasm32-wasip2 $(printf -- '-p %s ' "${PLUGINS[@]}")
    rm -f "$BUNDLED"/*.wasm
    for p in "${PLUGINS[@]}"; do
        cp "target/wasm32-wasip2/release/${p//-/_}.wasm" "$BUNDLED/"
    done
fi

if wants editor; then
    echo "==> editor"
    (cd ticknode-editor && trunk build)
fi

if wants server; then
    echo "==> server"
    cargo build -p ticknode-server
fi

if [ -f "$PIDFILE" ]; then
    pid=$(cat "$PIDFILE")
    if kill -0 "$pid" 2>/dev/null; then
        echo "==> stopping server ($pid)"
        kill "$pid"
        # SIGTERM makes the server save the graph; wait for it to exit
        while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done
    fi
    rm -f "$PIDFILE"
fi

export TICKNODE_DATA_DIR=$DATA
export TICKNODE_BUNDLED_PLUGINS_DIR=$BUNDLED
export TICKNODE_WEB_DIR=$ROOT/ticknode-editor/dist
export TICKNODE_BIND=${TICKNODE_BIND:-127.0.0.1:8099}
export RUST_LOG=${RUST_LOG:-info}

host=${TICKNODE_BIND%:*}
port=${TICKNODE_BIND##*:}
urls=()
if [ "$host" = 0.0.0.0 ]; then
    # Listening on every interface: localhost plus the machine's LAN addresses
    urls+=("http://localhost:$port")
    for ip in $(ip -4 -o addr show scope global 2>/dev/null | awk '{split($4, a, "/"); print a[1]}'); do
        urls+=("http://$ip:$port")
    done
else
    urls+=("http://$host:$port")
fi
echo
echo "================================================"
echo " TickNode editor:"
for url in "${urls[@]}"; do echo "   $url"; done
echo "================================================"
echo
echo $$ > "$PIDFILE"
exec "$ROOT/target/debug/ticknode-server"
