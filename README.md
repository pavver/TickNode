<div align="center">

# TickNode

**Visual, node-based automation that runs on a server and is edited in the browser.**

[![CI](https://github.com/pavver/TickNode/actions/workflows/ci.yml/badge.svg)](https://github.com/pavver/TickNode/actions/workflows/ci.yml)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
![Rust 2024](https://img.shields.io/badge/rust-2024-orange.svg)

</div>

TickNode lets you build automations by wiring nodes together instead of writing YAML or scripts. The engine runs continuously on a server, the editor is a web page, and new kinds of nodes come as sandboxed WebAssembly plugins.

Its first target is **Home Assistant**: TickNode is being developed into an add-on that can replace HA's built-in automations. Home Assistant support is a plugin, so TickNode works just as well on its own.

> [!WARNING]
> TickNode is in early development. The graph format, the plugin interface and the protocol still change without notice.

## Highlights

- **Predictable execution.** Work happens in ticks (100 ms by default). Execution impulses move one step per tick, so loops never freeze the engine; data is pulled on demand, and data cycles are rejected when you connect them.
- **Two kinds of wires.** *Exec* wires decide **when** something happens, *data* wires carry **values**. Pure nodes (math, comparisons, field access) compute only when their result is needed.
- **Strong types.** Numbers, strings, booleans, time of day, date-time, duration (in time such as `5s` or in ticks such as `10t`), colour, typed lists and objects — plus **types defined by plugins** (structures and enumerations) with generated *Make*/*Break* nodes, required inputs and struct pins that can be split into fields.
- **A decision node that reads like a sentence.** `Match` routes execution by conditions such as `temp > 25 and mode = "auto"`, with operators chosen by input type.
- **Sandboxed plugins.** Plugins are WebAssembly components run by wasmtime. They have no network or files unless granted a capability, every call has a CPU and memory budget, and a crashing plugin is restarted without stopping the engine.
- **Plugins managed from the UI.** Enable, disable and configure plugins in the editor; secrets stay on the server. Nodes of a disabled or missing plugin are kept as stubs, so the graph never loses them.
- **Everything edited on the node.** Properties, input values, types and conditions are set right on the canvas; the inspector shows the last execution of each node.
- **Comfortable editing.** Shared undo/redo, copy/paste, copying settings between nodes of one type, compatible node suggestions when a wire is dropped on empty canvas, and route points to lead wires around cards.
- **Safe updates.** Plugins carry a fingerprint of the plugin API, and an incompatible plugin is reported instead of crashing. When an update changes a node's pins, the node is frozen until you confirm it, and wires that no longer fit stay visible as detached wires instead of disappearing.
- **Git-friendly storage.** The graph is a stable, pretty-printed `data/graph.json`.
- **English and Ukrainian UI**, localized with Mozilla Fluent.
- **Rust end to end.** Engine, server, editor (Leptos → WebAssembly) and plugins share one set of types.

## Quick start

Requirements: a recent stable Rust toolchain with two extra targets, and [trunk](https://trunkrs.dev) for the editor.

```bash
rustup target add wasm32-wasip2 wasm32-unknown-unknown
cargo install trunk
```

Build the plugins, the editor and run the server:

```bash
# Plugins (WebAssembly components) go to data/plugins/
cargo build --release --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p ha-plugin
mkdir -p data/plugins && cp target/wasm32-wasip2/release/*_plugin.wasm data/plugins/

# Editor
(cd ticknode-editor && trunk build --release)

# Server
cargo run --release -p ticknode-server
```

Open <http://localhost:8099>.

For development, `./dev.sh` rebuilds the plugins, the editor and the server and restarts the server on a separate `.dev-data/` directory (`./dev.sh editor server` rebuilds only those parts).

Plugins must be rebuilt whenever the plugin API changes (the server reports such plugins in the *Plugins* panel).

The server is configured with environment variables:

| Variable | Default | |
|---|---|---|
| `TICKNODE_BIND` | `0.0.0.0:8099` | listen address |
| `TICKNODE_DATA_DIR` | `data` | graph, plugins, plugin data and settings |
| `TICKNODE_WEB_DIR` | `ticknode-editor/dist` | the built editor |
| `TICKNODE_BUNDLED_PLUGINS_DIR` | | extra read-only directory with `*.wasm` plugins, scanned in addition to `<data>/plugins` (a same-named file in `<data>/plugins` wins) |
| `TICKNODE_TICK_MS` | `100` | tick length |
| `TICKNODE_TICK_BUDGET` | unlimited | work units per tick; queued work beyond it moves to the next tick (`0` = unlimited) |
| `TICKNODE_PLUGIN_<PLUGIN>_<KEY>` | | default for a plugin setting when none is saved, e.g. `TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN` |
| `RUST_LOG` | | log filter |

## Home Assistant add-on

TickNode can run as a Home Assistant add-on with the editor in the sidebar (ingress) and an automatic
connection to Home Assistant. In Home Assistant open **Settings > Add-ons > Add-on store**, menu
(three dots) **> Repositories**, add `https://github.com/pavver/TickNode`, then install **TickNode**.
Prebuilt images are not published yet, so Home Assistant builds the add-on locally on install
(several minutes, network required). See [`ha-addon/ticknode/DOCS.md`](ha-addon/ticknode/DOCS.md).

## Writing a plugin

A plugin is a Rust `cdylib` built for `wasm32-wasip2` with [`ticknode-plugin-sdk`](ticknode-plugin-sdk):

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
ticknode-plugin-sdk = { git = "https://github.com/pavver/TickNode" }
```

```rust
use ticknode_plugin_sdk::*;

struct DoublePlugin;
struct Double;

impl PluginNode for Double {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "example/double".into(),
            title: "Double".into(),
            category: "Math".into(),
            description: "Multiplies a number by two".into(),
            help: "On a signal at In, outputs Val × 2 on Res and continues on Out.".into(),
            pins: vec![
                exec_in(0, "In"),
                exec_out(1, "Out"),
                data_in(2, "Val", DataType::Number),
                data_out(3, "Res", DataType::Number),
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let result = NumberValue::Float(inputs.number(PinId(2), 0.0) * 2.0).normalize();
        vec![Output::Data(PinId(3), DataValue::Number(result)), Output::Exec(PinId(1))]
    }
}

impl Plugin for DoublePlugin {
    const NODE_TYPES: &'static [&'static str] = &["example/double"];

    fn manifest() -> PluginManifest {
        PluginManifest::new("double_plugin", "0.1.0", "you")
    }

    fn create(type_name: &str, _node: NodeId) -> Option<Box<dyn PluginNode>> {
        (type_name == "example/double").then(|| Box::new(Double) as Box<dyn PluginNode>)
    }
}

export_plugin!(DoublePlugin);
```

Build it with `cargo build --release --target wasm32-wasip2` and drop the `.wasm` into `data/plugins/`. Plugins can also declare data types, settings, capabilities (HTTP, storage) and start host tasks (delays, HTTP requests); see [`delay-plugin`](delay-plugin), [`http-weather-plugin`](http-weather-plugin) and [`test-plugin`](test-plugin).

Write definitions (titles, descriptions, help, pin names) in English: they are the fallback text. A plugin ships translations as Fluent files (`const TRANSLATIONS = &[("uk", include_str!("../locales/uk.ftl"))]`, key conventions are in the [`ticknode-plugin-sdk`](ticknode-plugin-sdk) docs), and errors the user must see in the UI (for example from `configure`) are `UiError`s with a translation key and an English fallback. Plugin log messages must be in English (the UI is translated, logs are not).

## Repository layout

| Crate | |
|---|---|
| [`ticknode-shared`](ticknode-shared) | types shared by everything: values, node definitions, graph model, editor protocol |
| [`ticknode-core`](ticknode-core) | the tick engine, built-in nodes and the WebAssembly plugin host |
| [`ticknode-server`](ticknode-server) | runs the engine, stores the graph and serves the editor over WebSocket |
| [`ticknode-editor`](ticknode-editor) | the web editor (Leptos) |
| [`ticknode-plugin-sdk`](ticknode-plugin-sdk) | SDK for writing plugins |
| `*-plugin` | plugins: examples, `ha-plugin` (Home Assistant integration), `test-plugin` (used by the host tests) |
| [`wit/plugin.wit`](wit/plugin.wit) | the contract between the host and plugins |

How it works inside: [ARCHITECTURE.md](ARCHITECTURE.md).

## Roadmap

- [x] Server-side engine, Leptos editor, sandboxed WebAssembly plugins
- [x] Plugin-defined data types, required inputs
- [x] Plugins panel: enable, disable, configure
- [x] English and Ukrainian UI
- [x] Translations shipped by plugins; translatable UI errors
- [x] WebSocket capability for plugins
- [x] Home Assistant plugin (on [ha-core-client](https://github.com/pavver/ha-core-client)): triggers, conditions, actions and entity values
- [x] Home Assistant add-on: install by adding this repository to the add-on store (built locally on install)
- [x] Undo/redo, settings copy, wire suggestions and route points
- [x] Plugin API fingerprint; frozen nodes and detached wires after pin changes
- [ ] Prebuilt add-on images
- [ ] Editor redesign

## Contributing

Bug reports and ideas are welcome. Pull requests are accepted under the terms in [CONTRIBUTING.md](CONTRIBUTING.md), which allow the maintainer to relicense contributions.

## License

[Mozilla Public License 2.0](LICENSE). Modified files of TickNode itself must stay under MPL-2.0 and be available in source form when you distribute them. Plugins built with `ticknode-plugin-sdk` are separate works and can use any license, including a proprietary one.

The editor bundles the Cascadia Code font, © Microsoft Corporation, under the [SIL Open Font License 1.1](ticknode-editor/assets/CascadiaCode-OFL.txt).
