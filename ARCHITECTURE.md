# TickNode architecture

This document explains how TickNode works. The practical rules for changing the code (commands, conventions, pitfalls) are in [AGENTS.md](AGENTS.md).

```mermaid
flowchart LR
    subgraph Browser
        E[ticknode-editor<br/>Leptos CSR]
    end
    subgraph Server["ticknode-server (axum)"]
        WS[/ws session/]
        A[engine actor<br/>own OS thread]
        S[(data/graph.json<br/>data/plugin-config)]
    end
    subgraph Core["ticknode-core"]
        T[TickEngine]
        P[plugin host<br/>wasmtime]
    end
    W1[[plugin.wasm]]
    W2[[plugin.wasm]]

    E <-- postcard over WebSocket --> WS
    WS <--> A
    A --> T
    A <--> S
    T --> P
    P --> W1
    P --> W2
```

- **The server runs the graph.** The engine keeps running when no editor is open.
- **The editor only edits.** It sends commands and renders the state the server reports back; it never changes the graph locally.
- **Everything is Rust.** The engine, the server, the editor (compiled to WebAssembly) and the plugins (WebAssembly components) share the types from `ticknode-shared`.

## 1. Tick-based execution

Time is divided into ticks, 100 ms by default (`TICKNODE_TICK_MS`). One tick:

1. Finished asynchronous tasks (delays, HTTP requests) are delivered to the nodes that started them.
2. Every enabled plugin gets one `plugin-tick` call (WebSocket events in, emissions to its nodes out), then event sources are polled (clocks, triggers, plugin nodes that declared `polls`).
3. The execution queue runs.

An optional work budget (`TICKNODE_TICK_BUDGET`) limits how much work one tick may do: every node has a relative `cost`, and activations that do not fit move, in order, to the next tick. An idle tick only visits event sources and the nodes that need to stay live, so its cost does not grow with the size of the graph.

### Two flows

| | Execution flow | Data flow |
|---|---|---|
| Model | **push** | **pull** |
| Carries | "run now" impulses between exec pins | values between data pins |
| Timing | a fired exec output queues the next node for the **next** tick | before a node runs, its inputs are pulled recursively in the same tick |
| Cycles | allowed — each iteration takes a tick, so a loop never freezes the engine | forbidden — connecting a data pin that would create a cycle is rejected |

### Kinds of nodes

- **Active nodes** have exec inputs, may keep state and run when triggered.
- **Pure nodes** have no exec pins. They are evaluated on demand when another node pulls their outputs, and the result is cached for the rest of the tick.
- **Event sources** start chains by themselves (Clock, plugin triggers).

`NodeOutput::Clear(pin)` removes a data output value, invalidates pure results, and leaves that pin out of execution traces. Omitting an output keeps its previous value. Connected inputs without a source value receive no value; required inputs wait silently.

Global variables (`variable/set`, `variable/get`) share state between branches of the graph. Plugins cannot touch them directly, only through pins.

## 2. Data types

| Type | Notes |
|---|---|
| `bool` | |
| `number` | switches between integer and float automatically (`30 / 2` is the integer `15`) |
| `string` | |
| `time` | time of day, `07:30` |
| `datetime` | UTC timestamp |
| `duration` | wall-clock time (`90`, `1h 30m`, `500ms`) or engine ticks (`10t`); delay nodes convert either to ticks |
| `color` | `#rrggbb[aa]` |
| `list<T>` | typed list, may be nested |
| `object` | untyped set of named fields |
| `any` | accepts everything; operators adapt to the value |
| **named types** | declared by plugins, see below |

Connections accept exact compatibility or a shared `TypeRegistry::convertible` rule: text ↔ number/boolean/time/date-time/duration/colour, number ↔ boolean, and text → enum (only stored member values). Boolean text accepts common state pairs such as on/off, yes/no, open/closed and home/not_home; unknown text produces no value. During input pulling, the engine converts each connection value without changing the source cache. Failed conversions leave the input without a value; required inputs block execution and log once until recovery. Lists convert element by element; any invalid element makes the whole input unavailable. Structures are never unpacked implicitly. Converting wires are dashed and show localized source/destination types; the editor uses the same shared rules.

### Plugin-defined types

A plugin can declare its own **structures** (named fields with types, some optional) and **enumerations** (a fixed list of values). A value of a structure carries its type name, so the system knows what it is:

- pins are strictly typed: an `ha.state` input accepts only an `ha.state` (or `any`); a structure can go into an `object` input, but an untyped `object` cannot go into a structure input;
- for every structure the engine generates **Make** and **Break** nodes that build a value from fields and split it back;
- enumeration inputs show a drop-down of the allowed values right on the node;
- a type has its own colour and title in the editor.

Struct pins can also be split directly on a node. The engine stores `split:<pin>` as a generic property, projects output fields on pull, and assembles split inputs into tagged structures before calling the node. Non-optional fields are required; absent optional fields have no value. Effective definitions contain field pins in declaration order. Split/recombine removes connections to the replaced pins; settings survive graph saves, snippets and plugin stubs. `SetPinSplit { node, pin, split }` returns disconnect events followed by `NodeUpdated`, so the editor remains server-driven.

Virtual field ids reserve `0x8000_0000..=0xbfff_ffff`: `0x8000_0000 | (pin << 8) | field_index` (zero-based). Real pins cannot use this range; existing high ids such as HA Action’s `u32::MAX` outputs remain valid. Splitting currently supports one level, original ids up to `0x3f_ffff`, and 1–256 fields. Append fields to preserve connections. Make/Break nodes remain available for nested structures and explicit assembly. Pin type captions and tooltips use localized titles; technical names appear only as secondary tooltip text.

### Required inputs

A pin can be marked as required (every non-optional field of a Make node is). A node whose required input is neither connected nor set on the node does not run, and the editor highlights it.

### Pin changes after updates

Every node in the saved graph records the signature of its pins (id, direction, exec or data type, required). When a plugin or TickNode update changes them in a breaking way (a pin is gone, its kind or type changed, or a new required input appeared), the node is **frozen**: it does not run, poll or produce values until the user confirms the new pins in the editor. Connections that no longer fit are kept as **detached** connections, drawn from their live end to a loose end next to the old pin, so the user can reconnect or delete them. New optional pins and renamed pins are accepted silently.

## 3. Plugins

Third-party nodes run only inside WebAssembly components ([`wit/plugin.wit`](wit/plugin.wit)), executed by wasmtime:

- **Isolation.** A plugin gets no network, files or environment by default. It can only ask for **capabilities**: HTTP to listed domains (the host performs the request and does not follow redirects), WebSocket connections to listed hosts, or a private storage directory.
- **Limits.** Every call into a plugin has a fuel (CPU) and memory budget. A crash or an endless loop recreates the plugin instance; the other nodes are rebuilt from their saved properties and the engine keeps running.
- **Host-driven async.** Plugins never block: delays and HTTP requests are host tasks whose results arrive in a later tick.
- **Nodes are resources.** Every node in the graph is a separate resource inside the plugin instance, with its own state; it is created knowing its graph node id.
- **Plugin-level connections and events.** A plugin (not a node) can own up to 4 persistent WebSocket connections. The host runs them on its tokio runtime and, once per tick, calls the plugin's `plugin-tick` with the received events (text frames, opened/closed). The plugin answers with *emissions*: outputs for any of its own nodes, applied as if the node had executed. This lets one connection serve many nodes (for example a Home Assistant session). Connections die with the plugin instance (disable, crash); `configure` runs again on the new instance, which reconnects.
- **Types and settings.** A plugin declares its node types, its data types and a settings schema. Settings are edited in the editor's *Plugins* panel, stored on the server (secrets never leave the server) and passed to the plugin through `configure`.
- **Property schema and dynamic options.** A node can describe its string properties (title, a static drop-down, or a searchable picker). The options of a picker are published at runtime by the plugin (`set-options`) - for example the Home Assistant plugin lists its entities with icon, name and id - kept by the host per plugin, cleared when the plugin is disabled and sent to editors (`Init`, then only when a list changes).
- **Enable/disable at runtime.** Disabling a plugin turns its nodes into stubs that keep their properties and connections; enabling it brings them back. A graph that references a missing plugin keeps those nodes as stubs too, so nothing is lost on save.

- **API fingerprint.** `ticknode_shared::PLUGIN_API` is a hash of the WIT contract (without comments and whitespace) and of a snapshot of the `DataValue` serialization layout. The SDK embeds it in every plugin, and the host compares it before instantiating the plugin, so an outdated plugin gets a clear "rebuild the plugin" message instead of a low-level type error.

Plugins are written in Rust with [`ticknode-plugin-sdk`](ticknode-plugin-sdk); see `delay-plugin`, `math-plugin`, `string-plugin` and `http-weather-plugin` for examples.

## 4. Server and protocol

- **Engine actor.** The engine lives on a dedicated OS thread that processes ticks and editor commands one after another, so the engine needs no locks. (A dedicated thread is also required by wasmtime's synchronous WASI, which must not run inside a tokio task.)
- **Protocol.** postcard over WebSocket. After connecting, an editor receives a snapshot (`Init`); afterwards every change is broadcast to all editors as an event, so several editors stay in sync. Errors go only to the editor that caused them. Execution traces, logs and delay timers are sent at a fixed rate (every 100 ms) independent of the tick length.
- **History.** Undo/redo belongs to the server and is shared by all editors: every successful edit (or a batch of edits) stores a snapshot, and undo reconciles the live graph to it, recreating only the nodes that differ.
- **Persistence.** The graph is stored in `data/graph.json`: nodes with positions, optional user sizes and pin signatures, connections, detached connections and wire route points, pretty-printed with a stable order so that it diffs well in git, written atomically with a short debounce and on shutdown. A corrupt file stops the start-up instead of being overwritten.

## 5. Editor

- Built with Leptos, compiled to WebAssembly and served by the server.
- All node settings are edited on the node itself; the inspector shows the description, the last execution (inputs, outputs, fired pins) and actions.
- Described properties are drop-downs, or - for dynamic ones - a searchable picker (icon, name and id per row, keyboard navigation, capped rendering) in a popup layer above the canvas. Icons are Material Design Icons bundled with the editor (works offline).
- Localized with Mozilla Fluent (English and Ukrainian). The editor owns only its UI texts (`ui.ftl`). Texts of nodes, data types, categories and plugin settings come from the server as Fluent resources: the core embeds the translations of the built-in nodes, every plugin ships its own (the `translations` export). The server validates plugin resources (syntax, and every message id must start with one of the plugin's own prefixes), merges them per locale and sends them in `Init` and `Catalog`. Lookup order in the editor: active locale, `en`, then the English text in the definition.
- Errors for the user travel as a `UiMessage` (translation key, string arguments, English fallback); the editor resolves the key in its own and the server-provided texts. Logs are deliberately not translated: they stay in English so that a problem report from any locale can be read as-is.

## 6. Home Assistant

TickNode is meant to replace Home Assistant's automations, but Home Assistant support is **a plugin, not part of the core**: TickNode runs standalone as well. The `ha-plugin` is built on [ha-core-client](https://github.com/pavver/ha-core-client), whose sans-IO `Session` runs inside the plugin while the host owns the WebSocket connection. It provides trigger nodes for every HA trigger platform, condition gates, an action node with typed fields from the service catalog, entity value nodes and searchable entity/action pickers.

The add-on (`ha-addon/ticknode`, made installable by `repository.yaml`) runs the server with the editor in the Home Assistant sidebar through ingress, ships the plugins in the image, and connects the Home Assistant plugin to Core through the Supervisor proxy with the Supervisor token, so no setup is needed.
