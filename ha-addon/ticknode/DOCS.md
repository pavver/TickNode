# TickNode

TickNode is a node-based visual programming system for automation. This add-on runs the
TickNode server together with its web editor and plugs it into Home Assistant.

## Usage

1. Install and start the add-on.
2. Open **TickNode** in the Home Assistant sidebar (or use **Open web UI**). The editor is served
   through Home Assistant ingress, so Home Assistant authentication protects it; no port is exposed.
3. Build graphs from the palette. Nodes of the `Home Assistant` plugin (state changes, get state,
   call service) are available right away.

## How the Home Assistant connection works

The add-on is granted access to the Home Assistant Core API (`homeassistant_api: true`). On start
the Supervisor token is passed to the bundled Home Assistant plugin as its `token` setting, and the
plugin connects to `ws://supervisor/core/websocket` (the add-on sets this URL). Nothing has to be configured.
The token is never shown in the editor. You can still override the URL or token in the editor's
Plugins panel; saved values win over the automatic ones.

## Data and plugins

- `/data` is persistent and part of Home Assistant backups: the graph (`graph.json`), plugin
  settings (`plugin-config/`) and plugin files (`plugin-data/`).
- Bundled plugins ship with the add-on image and are updated together with it. Your own plugins
  (`*.wasm`) can be placed in `/data/plugins`; a file there with the same name as a bundled plugin
  replaces it.

## Standalone use

TickNode does not require Home Assistant: the same server can run on its own (see the project
README). Without the Supervisor the Home Assistant plugin needs an explicit URL and a long-lived
access token.

## Installation note

Prebuilt images are not published yet, so Home Assistant builds the add-on locally on
installation. This needs network access and takes several minutes (it compiles the server, the
plugins and the editor).
