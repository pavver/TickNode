# Changelog

## Unreleased (built from `master`)

- Editor: undo/redo, copying settings between nodes, node suggestions when a wire is dropped on
  empty canvas, route points on wires.
- Durations can be given in time (`5s`) or ticks (`10t`); delay nodes take one duration input and
  show a progress bar while waiting.
- Plugins carry a plugin API fingerprint; an incompatible plugin is reported in the Plugins panel.
- After an update that changes node pins, the node is frozen until confirmed and wires that no
  longer fit are kept as detached wires.
- Rebuilding the add-on always fetches the latest commit (no stale Docker cache).

## 0.1.0

- Initial add-on: TickNode server and editor behind Home Assistant ingress, bundled plugins
  (delay, string, math, http-weather, Home Assistant) and automatic Home Assistant connection.
