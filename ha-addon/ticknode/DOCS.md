# TickNode

TickNode is a node-based visual programming system for automation. This add-on runs the
TickNode server together with its web editor and plugs it into Home Assistant.

## Usage

1. Install and start the add-on.
2. Open **TickNode** in the Home Assistant sidebar (or use **Open web UI**). The editor is served
   through Home Assistant ingress, so Home Assistant authentication protects it; no port is exposed.
3. Build graphs from the palette. Nodes of the `Home Assistant` plugin are available right away:
   triggers for every Home Assistant trigger platform (state, numeric state, time, sun, zone,
   event, MQTT, webhook, template and others), condition gates, an **Action** node with fields
   from the service catalog, **Entity** value nodes, and lower-level state/service nodes. Entities
   and actions are chosen in searchable pickers.

## How the Home Assistant connection works

The add-on is granted access to the Home Assistant Core API (`homeassistant_api: true`). On start
the Supervisor token is passed to the bundled Home Assistant plugin as its `token` setting, and the
plugin connects to `ws://supervisor/core/websocket` (the add-on sets this URL). Nothing has to be configured.
The token is never shown in the editor. You can still override the URL or token in the editor's
Plugins panel; saved values win over the automatic ones.

## Data and plugins

- `/data` is persistent and part of Home Assistant backups: scenario sheets (`sheets/`),
  engine settings (`settings.json`), plugin settings (`plugin-config/`) and plugin files (`plugin-data/`).
  Tick length and work budget are configured directly on the Settings page in the editor.
- Bundled plugins ship with the add-on image and are updated together with it. Your own plugins
  (`*.wasm`) can be placed in `/data/plugins`; a file there with the same name as a bundled plugin
  replaces it.

## Standalone use

TickNode does not require Home Assistant: the same server can run on its own (see the project
README). Without the Supervisor the Home Assistant plugin needs an explicit URL and a long-lived
access token.

## Installation and updates

Prebuilt images are not published yet, so Home Assistant builds the add-on locally on
installation: it downloads the current `master` branch from GitHub and compiles the server, the
plugins and the editor. This needs network access, several minutes, and a few gigabytes of free
memory and disk space (the build on aarch64 devices has not been tested yet).

Until images are published the add-on version stays the same, so Home Assistant does not offer
updates by itself. To get the latest TickNode, open the add-on page and use **Rebuild**; your
graph and settings in `/data` are kept. After an update that changes node pins, affected nodes are
frozen until you confirm them in the editor (see the inspector), and wires that no longer fit are
shown as detached wires.
