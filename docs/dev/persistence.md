# Persistence and usage statistics

Developer reference split out of [AGENTS.md](../../AGENTS.md).
Keep it current when the described code changes.

- Frames (`Frame`, `FrameId`, `FrameColor`) are layout rectangles saved in `GraphModel.frames` sorted by id, loaded into the actor and dropped if invalid. The engine ignores frames.
- Graph snapshots and JSON include confirmed `pins` and `detached_connections` (sorted by exec, source and destination). Missing fields in legacy JSON default to unknown signatures and no detached wires; frozen state is derived from confirmed signatures and current definitions.
- **Scenario sheets persistence.** Graphs are organized into scenario sheets (`data/sheets.json` with metadata, and each sheet's graph in `data/sheets/<sheet-id>.json`). Legacy `data/graph.json` is automatically migrated on startup into a default "Main" sheet and renamed to `graph.json.migrated`. Deleted sheets move to `data/sheets/trash/<id>-<timestamp>.json`. Writes are pretty-printed, atomic (tmp + rename), debounced ~1 s and done on SIGTERM. A corrupt sheet or meta file fails the start-up (never overwritten silently).
- Plugins are `data/plugins/*.wasm`; their files live in `data/plugin-data/<name>/`; settings and state in `data/plugin-config/<name>.json` (all outside git; configs contain secrets).
- **Environment overrides for plugin settings.** `TICKNODE_PLUGIN_<PLUGIN>_<KEY>` (the plugin name and the setting key uppercased, every non-alphanumeric character becomes `_`; for example `TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN`) supplies a setting value when the stored config has none (a missing key or an empty string); stored values win. The text is parsed by the setting kind (number, bool `true/false/1/0/yes/no/on/off`, otherwise a string) and a value that does not fit is ignored with a warning. Secrets supplied this way still never reach the editor, and the panel treats them as set; the override is not written into the config file. This is how the Home Assistant add-on passes `SUPERVISOR_TOKEN` (`ticknode-server/src/plugins.rs`, `stored_with_env`).
- `data/plugin-config/<name>.json` is `{"enabled": bool, "settings": {key: DataValue}}`, written atomically with file mode 0600 (directory 0700). A plugin without a file is enabled with default settings. A corrupt file gives the `Error` state and is not overwritten silently (only by explicitly saving settings or enabling the plugin).
- **Binary media storage (blob store).**
  Media payloads are persisted under `<data dir>/blobs/` with one content file `<id>` and a small sidecar `<id>.json` per blob, written atomically via temporary files and rename.
  A single total quota of 200 MB is enforced across all blobs.
  When a new blob exceeds available space, the oldest blobs (by creation timestamp) are evicted until the new blob fits.
  Blobs larger than the total quota are rejected.
  On startup, orphan temporary files and files without valid sidecars are cleaned up.
  Blobs are internal to the host and are never exposed over a public URL.

# Usage statistics (`ticknode-shared/src/usage.rs`, `ticknode-core/src/usage.rs`)
- In-memory cheap accounting with O(1) accumulation on the hot path (zero allocation per tick or node charge).
- Tracks plugin fuel, calls, traps, node executions and spent cost units (top 64 nodes plus "other"), per-sheet aggregates, tick wall time, and deferred activations.
- Rolling windows maintain the last 60 one-minute buckets and last 48 one-hour buckets in bounded ring buffers.
- Persistence optionally appends hour buckets to `data/stats/usage-YYYY-MM.jsonl` (compact single-line JSON, pruned after 6 months), skipped when the engine was idle and disabled when `ServerSettings.usage_stats` is false.
- Protocol: `ClientMessage::GetUsage` queries snapshot `ServerMessage::Usage(UsageReport)` on demand without periodic broadcast.
- The server logs one English `info` line per completed hour with total ticks, tick times, fuel, and work.
