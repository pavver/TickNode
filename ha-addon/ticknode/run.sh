#!/bin/sh
# Entrypoint of the TickNode Home Assistant add-on.
set -eu

export TICKNODE_DATA_DIR=/data
export TICKNODE_BIND=0.0.0.0:8099
export TICKNODE_WEB_DIR=/opt/ticknode/web
# Plugins shipped with the image; they update with the add-on. User plugins go to /data/plugins.
export TICKNODE_BUNDLED_PLUGINS_DIR=/opt/ticknode/plugins
# The Home Assistant plugin talks to Core through the Supervisor proxy with the Supervisor token.
# These are defaults: a URL or token saved in the plugin settings still wins.
export TICKNODE_PLUGIN_HOME_ASSISTANT_URL=ws://supervisor/core/websocket
if [ -n "${SUPERVISOR_TOKEN:-}" ]; then
    export TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN="$SUPERVISOR_TOKEN"
fi

exec /usr/local/bin/ticknode-server
