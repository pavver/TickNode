#!/bin/sh
# Entrypoint of the TickNode Home Assistant add-on.
set -eu

export TICKNODE_DATA_DIR=/data
export TICKNODE_BIND=0.0.0.0:8099
export TICKNODE_WEB_DIR=/opt/ticknode/web
# Plugins shipped with the image; they update with the add-on. User plugins go to /data/plugins.
export TICKNODE_BUNDLED_PLUGINS_DIR=/opt/ticknode/plugins
# The Home Assistant plugin's default URL (ws://supervisor/core/websocket) fits the add-on;
# only the token comes from the Supervisor.
if [ -n "${SUPERVISOR_TOKEN:-}" ]; then
    export TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN="$SUPERVISOR_TOKEN"
fi

exec /usr/local/bin/ticknode-server
