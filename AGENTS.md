# AGENTS.md

Guidance for coding agents (Claude Code, Gemini / Antigravity, Codex, …) working in this repository. `CLAUDE.md` and `GEMINI.md` only import this file.

TickNode is a node-based visual programming system for automation. The goal is a Home Assistant add-on that replaces HA's built-in automations, while TickNode also runs standalone. Rust 2024 workspace. The concepts are described in `README.md` and `ARCHITECTURE.md`.

- The engine runs on the server (`ticknode-server`); the browser is only an editor (`ticknode-editor`, **Leptos** CSR). Plugins are WASM components (**wasmtime + WIT**, `wit/plugin.wit`). The editor↔server protocol is postcard; the types are shared through `ticknode-shared`. The graph is stored as JSON.
- **Integrations (Home Assistant included) are separate plugins, never part of the core or the server.** All third-party nodes run as WASM plugins with limited capabilities.

## Language rules

- Talk to the user in **Ukrainian**.
- Documentation (`*.md`) and code comments are in **English**.
- Keep every Rust source file at most 500 lines; extract focused modules or test files when needed.
- **Editor UI text** is localized with Mozilla Fluent (`en`, `uk`); never hard-code UI strings (see *Editor → Internationalization*).
- **Errors the user sees in the UI** (an answer to their action, a plugin settings error under the form, validation, the plugin state error, a stub node's explanation) must be translatable: they are a `ticknode_shared::UiMessage { key, args, fallback }` (Fluent key, string arguments, final English fallback), never a ready-made sentence. Engine errors map to `error-...` keys in the editor's `ui.ftl` (`EngineError::ui_message`); a plugin's own errors (`configure`) use keys from its translations. A message without a key (`UiMessage::plain`, `From<String>`) just shows the English text.
- **Definitions are English.** Titles, descriptions, help, pin names, type and setting texts in node/plugin code are English; they are the fallback. Other languages come only from Fluent resources (see *Translations*). Ukrainian words that the text parsers accept as input aliases (`ticknode_shared::text`, `condition`, keyboard shortcut letters) stay.
- **Logs are always English and are never translated**: the server log (`tracing`), node and plugin messages (`host::log`, `ServerMessage::Log`) and the editor console. This is deliberate: when a user reports a problem from any locale, their logs can be read without translation. Plugin authors must log in English; this includes `#[error(...)]` texts, `host::log`/`warn`/`error`, and `[UI]`/`[ERROR]` lines the editor writes to its console (plain English constants, no `t!`). Exceptions are server errors (`i18n::ui_text`) and the settings-copy confirmation (`settings-copied`): both answer a user action and are translated.

## Project rules (adopted from the Operator design system)

These rules come from [../operator-design-system/AGENTS.md](../operator-design-system/AGENTS.md) and apply to new and changed code.
A global redesign of the editor onto the design system's Leptos components (`web/operator-ui`) is planned for later, after the core functionality is complete.
Do not start that migration unless asked, but keep new UI code compatible with it (Design Units, tokens, control states).

### Formatting
* In Markdown files, every sentence is on its own line.
* Files are referenced with relative paths.

### Units and geometry (editor UI)
* Spatial values use Design Units (`1 DU = 4 logical points`, CSS `--du`, Rust `geometry::DU`).
* Font sizes and line heights use Type Units (`1 TU = 1 scalable logical point`).
* Raw `px`, `dp`, `sp`, `pt` and `rem` values are prohibited in new CSS and component code.
  The only exceptions are the unit definitions themselves and geometry measured at runtime (pointer positions, element sizes).
* Corner radius is `0 DU` unless the design system specification of the component lists a round exception.
* Standard border `0.25 DU`, emphasis border `0.5 DU`, always drawn inside the bounds.

### Tokens and icons
* Design tokens (colours, spacing, motion) are defined in [../operator-design-system/tokens/tokens.toml](../operator-design-system/tokens/tokens.toml); the editor mirrors them in `ticknode-editor/styles/base.css`.
  Do not invent new colour values in component CSS: add or reuse a token, and every theme must keep the same token set.
* The design system uses Material Symbols Sharp ([../operator-design-system/tokens/icons.toml](../operator-design-system/tokens/icons.toml)); the editor still bundles Material Design Icons (`mdi:*`), so prefer a symbol available in both and draw a custom icon only when nothing fits ([../operator-design-system/docs/foundations/iconography.md](../operator-design-system/docs/foundations/iconography.md)).

### Behavior
* Follow [../operator-design-system/docs/foundations/states.md](../operator-design-system/docs/foundations/states.md) for every interactive control.
* Pending blocks input, results do not, errors roll back with the same animation as the forward change.

### Rust
* No `.unwrap()` or `.expect()` in non-test code.
* Dates, times and durations use only `chrono` (`DateTime`, `NaiveTime`, `TimeDelta`); `std::time::Instant` remains only for monotonic engine measurements (error age, timers).
* Run `cargo clippy` (host crates, and the editor with `--target wasm32-unknown-unknown`) on the changed crates before committing; changed code must be warning-free.
* Run `cargo fmt --all` with the standard rustfmt settings (no `rustfmt.toml`) before committing; `cargo fmt --all --check` must pass.
* When formatting pushes a file over 500 lines, split it into focused modules; never change formatting rules to stay under the limit.

### Releases
* Before every release (version bump, add-on version change, tag or published image), run a ub-mode pass over the changes since the previous release: the reduced principles First Make it Work, Then Make it Right, Follow Practice (Claude Code: the `ub-mode` skill).
* The pass includes the unclebob metrics tools (`crapper`, `dryer`, `mutator`, run one crate at a time; results in `.metrics/`) and the 500-line limit for every Rust file.
* Fix or explicitly accept every finding before releasing.

### Git
* Conventional Commits in English (`feat(editor): ...`, `fix(core): ...`).
* No automated or AI signatures in commit messages.
* Commit messages, PR descriptions and tags never mention AI assistants or agents (Claude, Codex, Gemini/Antigravity, …): no `Co-Authored-By` trailers, "generated with" lines or similar. Existing commits stay as they are.

## Commands

```bash
cargo build
cargo test -p ticknode-core -p ticknode-shared -p ticknode-server
cargo test -p ticknode-core --test plugins delay_queue -- --nocapture   # a single test
cargo test -p ha-plugin                                                   # the HA plugin's own logic on the host target (no wasm needed)
cargo test -p telegram-plugin                                             # scripted Telegram API, injected clock and storage (no sockets)

# Engine benchmarks (Criterion; optionally uses target/wasm32-wasip2/debug/math_plugin.wasm).
cargo bench -p ticknode-core --bench engine

# Plugins are wasm32-wasip2 components. The integration tests (ticknode-core/tests/plugins.rs) load
# target/wasm32-wasip2/debug/*.wasm and are SKIPPED when the files are missing (TICKNODE_REQUIRE_PLUGINS=1 makes them fail)
cargo build --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p test-plugin -p ha-plugin -p telegram-plugin
# The Telegram engine test needs a separate test-api fixture (see telegram-plugin/README.md); CI builds it like this
cargo build --target wasm32-wasip2 -p telegram-plugin --features test-api && cp target/wasm32-wasip2/debug/telegram_plugin.wasm target/wasm32-wasip2/debug/telegram_plugin_test.wasm && cargo build --target wasm32-wasip2 -p telegram-plugin
# The server uses release builds copied to data/plugins/
cargo build --release --target wasm32-wasip2 -p math-plugin && cp target/wasm32-wasip2/release/math_plugin.wasm data/plugins/

# Local dev loop: rebuild plugins/editor/server and restart the server on .dev-data/ (reload the browser)
./dev.sh                 # or a subset: ./dev.sh editor server

# Server (env: TICKNODE_BIND=0.0.0.0:8099, TICKNODE_DATA_DIR=data, TICKNODE_WEB_DIR=ticknode-editor/dist; settings in data/settings.json fall back to TICKNODE_TICK_MS=100, TICKNODE_TICK_BUDGET=0, RUST_LOG)
cargo run -p ticknode-server

# Editor (Leptos CSR, trunk). Not in default-members: `cargo build` at the root does not build it
cd ticknode-editor && trunk build            # → dist/, served by the server
cd ticknode-editor && trunk serve            # dev on :8080, /ws is proxied to 127.0.0.1:8099
cd ticknode-editor && trunk build --release  # needs wasm-opt (trunk downloads it)
cargo test -p ticknode-editor --target x86_64-unknown-linux-gnu   # geometry and i18n tests (no browser)
```

- The `wasm-bindgen` version in `ticknode-editor/Cargo.toml` must match the wasm-bindgen-cli used by trunk.
- In the dev profile dependencies are built with `opt-level = 2`: unoptimized Cranelift compiles plugins tens of times slower.
- Never run the server against the real `data/` directory for experiments: it rewrites `data/sheets/` and `data/sheets.json`. Use a copy (`TICKNODE_DATA_DIR`).
- When the graph format or the protocol changes, update the types in `ticknode-shared` (shared by the server and the editor).
- The editor's tests depend on `ha-plugin` and the SDK: after SDK or WIT changes run them too.
- After pushing, check CI (`gh run list`, `gh run watch`): it builds every plugin and runs the plugin tests with `TICKNODE_REQUIRE_PLUGINS=1`.

## Architecture in brief

Read this section first; the detailed reference lives in `docs/dev/` (see *Where to look*).
When you change behaviour described there, update the matching file in the same commit.

### Crates
- `ticknode-shared` — every serialized type shared by the server and the editor (values, definitions, graph, protocol, manifest, types, variables, sheets, `UiMessage`), plus pure logic used on both sides: text parsing, Match conditions, quantities, To text, MiniJinja templates, the formula engine.
- `ticknode-core` — the engine: `TickEngine`, nodes and the registry, built-in nodes, host services (tasks, HTTP, blobs, logs), variables, WASM plugin runtime (wasmtime + WIT), translations.
- `ticknode-server` — axum server: `/ws` (postcard) and the editor's static files; the engine runs on its own OS thread (`actor.rs`), sheets and plugin configs are stored on disk.
- `ticknode-editor` — Leptos CSR editor; it only sends `ClientMessage`s and renders server state.
- `ticknode-plugin-sdk` — plugin SDK (wit-bindgen, `export_plugin!`).
- `*-plugin` — plugins; `test-plugin` exists only for host tests. `ha-plugin` (Home Assistant) and `telegram-plugin` are full integrations.

### Invariants that are easy to break
- A node is recreated only from `type_name` + `properties()`; `set_property` and `properties()` must be symmetric. Engine-owned per-node settings are stored as prefixed properties never passed to the node: `in:<pin>` (inline value), `split:<pin>`, `var:<pin>` (variable binding).
- Postcard is positional and the WIT is the plugin ABI: append new enum variants, struct fields and `Init` fields only at the end. A WIT change changes `PLUGIN_API`: rebuild every plugin (and the Telegram test fixture, see `telegram-plugin/README.md`).
- The server is the source of truth: the editor never changes the graph locally (exceptions: positions, sizes and route points while dragged); events may repeat, so apply them idempotently.
- Exec flow is push (next tick), data flow is pull with per-tick pure caches. Never insert into `TickEngine::nodes` directly: indexes and caches are maintained by `add_node`, `remove_node`, `set_node_property`, `set_pin_split` and registration.
- Synchronous wasmtime-wasi must never be called from a tokio task; the engine lives on a plain OS thread and plugin tests are plain `#[test]`s.
- User-facing errors are `UiMessage` (Fluent key, args, English fallback); logs are English and never translated; editor UI strings use `t!` with keys in both locales.
- Integrations (Home Assistant included) are plugins with limited capabilities, never part of the core or the server. Raw binary data never travels inside `DataValue`: media values reference the host blob store.
- Secrets (secret plugin settings, tokens in URLs) never reach the editor, traces, outputs or logs.

### Where to look
- [docs/dev/crates.md](docs/dev/crates.md) — module map of every crate and plugin.
- [docs/dev/nodes.md](docs/dev/nodes.md) — nodes and the graph: pins, frozen nodes, detached wires, split pins, types, quantities, templates, formulas, Match, built-in node families, properties and pickers.
- [docs/dev/execution.md](docs/dev/execution.md) — the tick, work budget, data pull, variables, tasks.
- [docs/dev/plugins.md](docs/dev/plugins.md) — WASM plugins: WIT, capabilities, HTTP, WebSockets, blobs, settings, pure nodes, enable/disable, stubs.
- [docs/dev/protocol.md](docs/dev/protocol.md) — editor protocol, history, batches, sheets routing, telemetry.
- [docs/dev/editor.md](docs/dev/editor.md) — editor behaviour, canvas, panels, i18n, layout geometry.
- [docs/dev/translations.md](docs/dev/translations.md) — Fluent resources of the core and plugins, key conventions, validation.
- [docs/dev/persistence.md](docs/dev/persistence.md) — data directory layout, sheets, plugin configs, blobs, usage statistics.
- [docs/dev/telegram.md](docs/dev/telegram.md) and [telegram-plugin/README.md](telegram-plugin/README.md) — the Telegram plugin.
- [docs/dev/home-assistant.md](docs/dev/home-assistant.md) — the Home Assistant add-on.
- [ARCHITECTURE.md](ARCHITECTURE.md) — concepts for humans; [README.md](README.md) — the project overview.
