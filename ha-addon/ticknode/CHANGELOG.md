# Changelog

## 0.2.0

First release with prebuilt images (`amd64`, `aarch64`): Home Assistant now offers updates.

- Editor: undo/redo, copying settings between nodes, node suggestions when a wire is dropped on
  empty canvas, route points on wires.
- Scenario sheets with their own canvas, enable/disable and history.
- Typed runtime variables with Sheet, Global and Persistent scopes and nodes to create, get, set,
  delete and react to changes. Any unconnected input can be bound to a variable by typing `var.x`,
  `sheet.x` or `global.x`.
- Text templates (Jinja style, with autocompletion), formulas, physical quantities with units,
  date and time nodes.
- Durations can be given in time (`5s`) or ticks (`10t`); delay nodes take one duration input and
  show a progress bar while waiting.
- Telegram bot plugin: triggers, messages built from text, media and inline or reply keyboards,
  editing messages, sending media and downloading files.
- Media: Download media fetches a file (for example a camera snapshot) over HTTP into a local
  store (200 MB, oldest files removed first) so it can be forwarded, for example to Telegram.
- Universal Make object / Break object nodes replace the per-type nodes; existing per-type nodes
  are removed.
- Plugins carry a plugin API fingerprint; an incompatible plugin is reported in the Plugins panel.
  Plugins can declare pure nodes.
- After an update that changes node pins, the node is frozen until confirmed and wires that no
  longer fit are kept as detached wires.
- Request URLs and their secrets (tokens, passwords) are kept out of error messages.

## 0.1.0

- Initial add-on: TickNode server and editor behind Home Assistant ingress, bundled plugins
  (delay, string, math, http-weather, Home Assistant) and automatic Home Assistant connection.
