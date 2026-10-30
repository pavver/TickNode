//! WASM plugins: wasmtime components implementing the `wit/plugin.wit` contract.
//!
//! One component instance (`Store`) per plugin; each graph node is a `node` resource within it,
//! so node state persists across calls. Calls are bounded by fuel and memory limits. If a plugin
//! traps (crash, out of fuel), the instance is recreated and nodes are reconstructed from their
//! properties on the next call. The node that caused the failure is halted until a property changes
//! so it does not trap on every tick.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ticknode_shared::*;
use tracing::warn;
use wasmtime::component::{Component, HasSelf, Linker, ResourceAny, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};
use wasmtime_wasi::{FsPerms, WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use crate::translations::{AllowedIds, filter_plugin_resource, normalize_locale, sanitize_ui_message};
use crate::ws::{self, WsHub};
use crate::host::{HostState, HttpRequest, TaskId, TaskOutput, TaskResult};
use crate::{Node, NodeContext, RemovedConnections, TickEngine};

mod bindings {
    wasmtime::component::bindgen!({ path: "../wit", world: "plugin" });
}

use bindings::exports::ticknode::plugin::nodes::Guest as NodesApi;
use bindings::ticknode::plugin::types as wit;

/// Instruction budget per plugin call (guards against infinite loops).
const FUEL_PER_CALL: u64 = 2_000_000_000;
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
/// Pause before `plugin-tick` is called again after the plugin trapped in it.
const TICK_BACKOFF_TICKS: u64 = 50;

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.consume_fuel(true);
        Engine::new(&config).expect("the wasmtime configuration is valid")
    })
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// --- Plugin instance state ---

/// Capabilities granted to the plugin at load time.
#[derive(Default)]
struct Grants {
    name: String,
    http_domains: Vec<String>,
    ws_hosts: Vec<String>,
    storage_dir: Option<PathBuf>,
}

/// Currently executing node: owns spawned tasks and log messages.
struct CallCtx {
    node_id: NodeId,
    host: HostState,
}

struct PluginState {
    wasi: WasiCtx,
    table: ResourceTable,
    limits: StoreLimits,
    grants: Arc<Grants>,
    call: Option<CallCtx>,
    /// Plugin-owned WebSocket connections; they die with this instance.
    ws: WsHub,
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl PluginState {
    fn call(&mut self) -> Result<&mut CallCtx, String> {
        self.call.as_mut().ok_or_else(|| "Only available during execute/poll".to_string())
    }
}

impl wit::Host for PluginState {}

impl bindings::ticknode::plugin::host::Host for PluginState {
    fn log(&mut self, level: wit::LogLevel, message: String) {
        let message = match level {
            wit::LogLevel::Debug | wit::LogLevel::Info => message,
            wit::LogLevel::Warn => format!("[WARN] {message}"),
            wit::LogLevel::Error => format!("[ERROR] {message}"),
        };
        match &mut self.call {
            Some(c) => c.host.log(c.node_id, message),
            None => warn!(plugin = %self.grants.name, "{}", message),
        }
    }

    fn current_tick(&mut self) -> u64 {
        self.call.as_ref().map_or(0, |c| c.host.current_tick)
    }

    fn tick_duration_ms(&mut self) -> u32 {
        self.call.as_ref().map_or(0, |c| c.host.tick_duration.as_millis() as u32)
    }

    fn delay(&mut self, ticks: u64) -> Result<TaskId, String> {
        let c = self.call()?;
        Ok(c.host.start_delay(c.node_id, ticks))
    }

    fn start_http(&mut self, request: wit::HttpRequest) -> Result<TaskId, String> {
        let url = url::Url::parse(&request.url).map_err(|e| format!("Invalid URL: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!("Unsupported scheme: {}", url.scheme()));
        }
        let host = url.host_str().unwrap_or_default();
        if !self.grants.http_domains.iter().any(|d| domain_allowed(host, d)) {
            return Err(format!("The plugin is not allowed to make HTTP requests to {host}"));
        }
        let request = HttpRequest {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout: std::time::Duration::from_millis(request.timeout_ms as u64),
        };
        let c = self.call()?;
        c.host.start_http(c.node_id, request)
    }

    fn cancel_task(&mut self, id: TaskId) -> bool {
        self.call().is_ok_and(|c| c.host.cancel_task(c.node_id, id))
    }

    fn ws_connect(&mut self, url: String, headers: Vec<(String, String)>) -> Result<wit::WsId, String> {
        let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;
        if !matches!(parsed.scheme(), "ws" | "wss") {
            return Err(format!("Unsupported scheme: {} (expected ws or wss)", parsed.scheme()));
        }
        let host = parsed.host_str().unwrap_or_default();
        if !self.grants.ws_hosts.iter().any(|d| domain_allowed(host, d)) {
            return Err(format!("The plugin is not allowed to open WebSocket connections to {host}"));
        }
        self.ws.connect(&url, &headers)
    }

    fn ws_send(&mut self, id: wit::WsId, text: String) -> Result<(), String> {
        self.ws.send(id, text)
    }

    fn ws_close(&mut self, id: wit::WsId) {
        self.ws.close(id);
    }
}

/// `example.com` allows the domain itself and its subdomains, but not `evilexample.com`.
fn domain_allowed(host: &str, allowed: &str) -> bool {
    allowed == "*" || host == allowed || host.ends_with(&format!(".{}", allowed))
}

// --- Plugin instance ---

struct Runtime {
    pre: bindings::PluginPre<PluginState>,
    grants: Arc<Grants>,
    /// Which Fluent ids the plugin may define; also its UI error key namespace.
    allowed: AllowedIds,
    /// `None` after a trap: a new instance is created on the next call.
    instance: Option<(Store<PluginState>, bindings::Plugin)>,
    /// Incremented with each new instance; resources from older instances are invalid.
    generation: u64,
    /// Settings to apply to each new instance (`configure`) before recreating nodes.
    settings: Option<Vec<wit::Property>>,
    /// Result of the last `configure` called during instance creation.
    configure_error: Option<UiMessage>,
    /// Runtime for the instance's WebSocket tasks (`HostState::runtime`, refreshed by the engine).
    tokio: Option<tokio::runtime::Handle>,
    /// After a trap in `plugin-tick`, the next call is made no earlier than this tick (a plugin that traps
    /// on every tick would otherwise be recreated and reconnected every tick).
    tick_backoff_until: u64,
}

impl Runtime {
    fn instantiate(&mut self) -> wasmtime::Result<u64> {
        if self.instance.is_none() {
            let mut wasi = WasiCtxBuilder::new();
            // Plugin panics (stderr) are visible in the server log.
            wasi.inherit_stderr();
            if let Some(dir) = &self.grants.storage_dir {
                wasi.preopened_dir(dir, ".", FsPerms::ReadWrite)?;
            }
            let state = PluginState {
                wasi: wasi.build(),
                table: ResourceTable::new(),
                limits: StoreLimitsBuilder::new().memory_size(MEMORY_LIMIT).build(),
                grants: self.grants.clone(),
                call: None,
                ws: WsHub::new(self.tokio.clone()),
            };
            let mut store = Store::new(engine(), state);
            store.limiter(|s| &mut s.limits);
            store.set_fuel(FUEL_PER_CALL)?;
            let plugin = self.pre.instantiate(&mut store)?;
            // A recreated instance after a crash receives settings first — otherwise it would lose its config
            if let Some(settings) = &self.settings {
                store.set_fuel(FUEL_PER_CALL)?;
                self.configure_error = plugin.ticknode_plugin_nodes().call_configure(&mut store, settings)?.err().map(|e| self.ui_error(e));
            }
            self.instance = Some((store, plugin));
            self.generation += 1;
        }
        Ok(self.generation)
    }

    fn api(&mut self) -> wasmtime::Result<(&mut Store<PluginState>, &NodesApi)> {
        self.instantiate()?;
        let (store, plugin) = self.instance.as_mut().expect("the instance was just created");
        store.data_mut().ws.set_runtime(self.tokio.clone());
        store.set_fuel(FUEL_PER_CALL)?;
        Ok((store, plugin.ticknode_plugin_nodes()))
    }

    /// Stores settings and applies them: to an existing instance via `configure`,
    /// while a new instance receives them upon creation. `Err` is the plugin's response (or a trap).
    fn configure(&mut self, settings: &[(String, DataValue)], tokio: Option<tokio::runtime::Handle>) -> Result<(), UiMessage> {
        self.tokio = tokio;
        let props: Vec<wit::Property> = settings.iter()
            .map(|(name, value)| wit::Property { name: name.clone(), value: encode_value(value) })
            .collect();
        self.settings = Some(props.clone());
        let fresh = self.instance.is_none();
        self.instantiate().map_err(|e| crash_message(&short_error(&e)))?;
        if fresh {
            return self.configure_error.clone().map_or(Ok(()), Err);
        }
        let outcome = match self.api() {
            Ok((store, api)) => api.call_configure(store, &props),
            Err(e) => Err(e),
        };
        match outcome {
            Ok(result) => {
                let result = result.map_err(|e| self.ui_error(e));
                self.configure_error = result.clone().err();
                result
            }
            Err(e) => {
                warn!("Plugin crashed during configure: {e:?}");
                let reason = short_error(&e);
                self.discard();
                Err(crash_message(&reason))
            }
        }
    }

    /// Delivers connection events and collects the plugin's emissions. On a trap the instance is discarded
    /// (its connections close, `configure` runs again on the next instance).
    fn plugin_tick(&mut self, host: &mut HostState) -> Result<Vec<wit::Emission>, String> {
        if host.current_tick < self.tick_backoff_until {
            return Ok(Vec::new());
        }
        self.tokio = host.runtime.clone();
        let outcome = self.api().map(|_| ()).and_then(|()| {
            let (store, plugin) = self.instance.as_mut().expect("api() creates the instance");
            let events: Vec<wit::WsEvent> = store.data_mut().ws.drain(ws::MAX_EVENTS_PER_TICK).into_iter().map(|e| match e {
                ws::WsEvent::Opened(id) => wit::WsEvent::Opened(id),
                ws::WsEvent::Message(id, text) => wit::WsEvent::Message((id, text)),
                ws::WsEvent::Closed(id, reason) => wit::WsEvent::Closed((id, reason)),
            }).collect();
            store.set_fuel(FUEL_PER_CALL)?;
            // Plugin-level logs are not tied to a node: they carry the nil node id
            store.data_mut().call = Some(CallCtx { node_id: NodeId(uuid::Uuid::nil()), host: std::mem::take(host) });
            let result = plugin.ticknode_plugin_nodes().call_plugin_tick(&mut *store, &events);
            *host = store.data_mut().call.take().expect("the call context is in place").host;
            result
        });
        outcome.map_err(|e| {
            warn!("Plugin crashed during plugin-tick: {e:?}");
            self.discard();
            self.tick_backoff_until = host.current_tick + TICK_BACKOFF_TICKS;
            short_error(&e)
        })
    }

    /// A `configure` error from the plugin; its key is restricted to the plugin's own namespace.
    fn ui_error(&self, e: wit::UiError) -> UiMessage {
        let message = UiMessage { key: e.key, args: e.args, fallback: e.fallback };
        sanitize_ui_message(message, &self.allowed)
    }

    /// Discards the instance after a trap. The generation number is incremented immediately so resources
    /// from the dead `Store` are never passed to a new instance (in particular from `WasmNode::drop`).
    fn discard(&mut self) {
        self.instance = None;
        self.generation += 1;
    }
}

// --- Plugin node ---

pub struct WasmNode {
    id: NodeId,
    type_name: String,
    runtime: Arc<Mutex<Runtime>>,
    /// Resource in the plugin instance with its `generation`; created on first use.
    handle: Option<(u64, ResourceAny)>,
    definition: NodeDefinition,
    /// Event source node (`poll` on every tick). Other nodes avoid paying WASM call overhead each tick.
    polls: bool,
    properties: Vec<(String, DataValue)>,
    failed: bool,
}

enum CallError {
    /// Instance trapped: it has been discarded and the node is halted.
    Trap(String),
    /// Rejected by the plugin (e.g., unknown node type).
    Rejected(String),
}

/// Concise explanation for the editor console; full wasm backtrace goes to the server log.
fn trap_error(e: wasmtime::Error) -> CallError {
    warn!("Plugin crashed: {e:?}");
    CallError::Trap(short_error(&e))
}

fn short_error(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<wasmtime::Trap>() {
        Some(wasmtime::Trap::OutOfFuel) => "per-call compute limit exceeded (infinite loop?)".to_string(),
        Some(wasmtime::Trap::UnreachableCodeReached) => "panic in the plugin (see the server log for details)".to_string(),
        Some(trap) => trap.to_string(),
        None => e.root_cause().to_string(),
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Trap(e) => write!(f, "Plugin crashed: {e}"),
            CallError::Rejected(e) => write!(f, "{e}"),
        }
    }
}

impl WasmNode {
    fn new(id: NodeId, definition: NodeDefinition, polls: bool, runtime: Arc<Mutex<Runtime>>) -> Self {
        Self { id, type_name: definition.type_name.clone(), runtime, handle: None, definition, polls, properties: Vec::new(), failed: false }
    }

    /// Calls a method on the node resource. If `host` is passed, it is moved into the plugin for the
    /// duration of the call (tasks, logs) and restored afterward.
    fn call<R>(
        &mut self,
        mut host: Option<&mut HostState>,
        f: impl FnOnce(&mut Store<PluginState>, &NodesApi, ResourceAny) -> wasmtime::Result<R>,
    ) -> Result<R, CallError> {
        if self.failed {
            return Err(CallError::Rejected("The node is stopped after a plugin error; change a property to restart it".into()));
        }
        let runtime = self.runtime.clone();
        let mut rt = lock(&runtime);
        let generation = rt.instantiate().map_err(trap_error)?;
        let (store, api) = rt.api().map_err(trap_error)?;

        let resource = match self.handle {
            Some((g, r)) if g == generation => Ok(r),
            _ => create_resource(store, api, &self.type_name, self.id, &self.properties),
        };
        let result = match resource {
            Ok(resource) => {
                self.handle = Some((generation, resource));
                if let Some(h) = &mut host {
                    store.data_mut().call = Some(CallCtx { node_id: self.id, host: std::mem::take(*h) });
                }
                let result = f(store, api, resource);
                if let Some(h) = host {
                    *h = store.data_mut().call.take().expect("the call context is in place").host;
                }
                result.map_err(trap_error)
            }
            Err(e) => Err(e),
        };
        if let Err(e) = &result {
            self.failed = true;
            if matches!(e, CallError::Trap(_)) {
                rt.discard();
            }
        }
        result
    }

    fn refresh(&mut self) -> Result<(), CallError> {
        let (definition, properties) = self.call(None, |store, api, node| {
            Ok((api.node().call_definition(&mut *store, node)?, api.node().call_properties(&mut *store, node)?))
        })?;
        (self.definition, self.polls) = from_wit_definition(definition);
        self.properties = properties.into_iter().filter_map(|p| Some((p.name, decode_value(&p.value)?))).collect();
        Ok(())
    }

    fn run(&mut self, trigger: wit::Trigger, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let inputs: Vec<(u32, wit::Value)> = ctx.inputs.iter()
            .filter_map(|(pin, value)| Some((pin.0, encode_value(value.as_ref()?))))
            .collect();
        let result = self.call(Some(ctx.host_state), |store, api, node| api.node().call_execute(store, node, &trigger, &inputs));
        match result {
            Ok(outputs) => outputs.into_iter().filter_map(|o| match o {
                wit::Output::Exec(pin) => Some(NodeOutput::Execution(PinId(pin))),
                wit::Output::Data((pin, value)) => {
                    let value = decode_value(&value)?;
                    match check_output_type(&self.definition, PinId(pin), &value) {
                        Ok(()) => Some(NodeOutput::Data(PinId(pin), value)),
                        Err(e) => {
                            ctx.host_state.log(self.id, format!("[ERROR] {e}"));
                            None
                        }
                    }
                }
            }).collect(),
            Err(e) => {
                ctx.host_state.log(self.id, format!("[ERROR] {e}"));
                Vec::new()
            }
        }
    }
}

/// A value emitted on a custom struct output pin must carry the tag of that exact type: a plugin
/// cannot emit another type or an untyped object disguised as `Named`.
fn check_output_type(definition: &NodeDefinition, pin: PinId, value: &DataValue) -> Result<(), String> {
    let declared = definition.pins.iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Output)
        .and_then(|p| match &p.pin_type {
            PinType::Data(DataType::Named(name)) => Some(name),
            _ => None,
        });
    match (declared, value) {
        (Some(name), DataValue::Object { type_name, .. }) if type_name.as_deref() != Some(name) => Err(format!(
            "The plugin emitted a value {} on an output of type {name}: the value was dropped",
            type_name.as_deref().map_or("without a type".to_string(), |t| format!("of type {t}")),
        )),
        _ => Ok(()),
    }
}

/// Checks the outputs a plugin pushed for a node outside of `execute`: each pin must be an output pin of the
/// matching kind (exec or data) and values must pass `check_output_type`. Returns the valid outputs and an
/// English error for every rejected one.
fn validate_emission_outputs(definition: &NodeDefinition, outputs: Vec<wit::Output>) -> (Vec<NodeOutput>, Vec<String>) {
    let output_pin = |pin: u32| definition.pins.iter().find(|p| p.id == PinId(pin) && p.direction == PinDirection::Output);
    let (mut valid, mut errors) = (Vec::new(), Vec::new());
    for output in outputs {
        match output {
            wit::Output::Exec(pin) => match output_pin(pin) {
                Some(p) if p.pin_type == PinType::Exec => valid.push(NodeOutput::Execution(PinId(pin))),
                _ => errors.push(format!("The plugin emitted an exec pulse on pin {pin}, which is not an exec output of {}", definition.type_name)),
            },
            wit::Output::Data((pin, bytes)) => match output_pin(pin) {
                Some(p) if p.pin_type != PinType::Exec => {
                    let Some(value) = decode_value(&bytes) else { continue };
                    match check_output_type(definition, PinId(pin), &value) {
                        Ok(()) => valid.push(NodeOutput::Data(PinId(pin), value)),
                        Err(e) => errors.push(e),
                    }
                }
                _ => errors.push(format!("The plugin emitted a value on pin {pin}, which is not a data output of {}", definition.type_name)),
            },
        }
    }
    (valid, errors)
}

fn create_resource(
    store: &mut Store<PluginState>,
    api: &NodesApi,
    type_name: &str,
    id: NodeId,
    properties: &[(String, DataValue)],
) -> Result<ResourceAny, CallError> {
    let node = api.call_create_node(&mut *store, type_name, id.0.as_u64_pair()).map_err(trap_error)?.map_err(CallError::Rejected)?;
    for (name, value) in properties {
        api.node().call_set_property(&mut *store, node, name, &encode_value(value)).map_err(trap_error)?;
    }
    Ok(node)
}

impl Node for WasmNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition { self.definition.clone() }

    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        match trigger_pin {
            Some(pin) => self.run(wit::Trigger::Pin(pin.0), ctx),
            None => Vec::new(),
        }
    }

    fn on_task(&mut self, task: TaskId, result: TaskResult, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let result = result.map(|output| match output {
            TaskOutput::Delay => wit::TaskOutput::Delay,
            TaskOutput::Http(r) => wit::TaskOutput::Http(wit::HttpResponse { status: r.status, headers: r.headers, body: r.body }),
        });
        self.run(wit::Trigger::Task(wit::TaskDone { id: task, result }), ctx)
    }

    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
        if !self.polls {
            return None;
        }
        let tick = host.current_tick;
        let id = self.id;
        match self.call(Some(&mut *host), |store, api, node| api.node().call_poll(store, node, tick)) {
            Ok(pin) => pin.map(PinId),
            Err(CallError::Rejected(_)) => None,
            Err(e) => {
                host.log(id, format!("[ERROR] {e}"));
                None
            }
        }
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        // Store the value immediately: it is used to reconstruct the node after a plugin restart.
        match self.properties.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value.clone(),
            None => self.properties.push((name.to_string(), value.clone())),
        }
        self.failed = false;
        let encoded = encode_value(&value);
        let result = self.call(None, |store, api, node| api.node().call_set_property(store, node, name, &encoded))
            .and_then(|()| self.refresh());
        if let Err(e) = result {
            warn!(node = ?self.id, r#type = %self.type_name, "The plugin did not accept property {name}: {e}");
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> { self.properties.clone() }

    /// A node created without properties (from the palette) would otherwise expose none until the first
    /// `set_property`: read the plugin's full set, defaults included, so the editor can show and edit them.
    fn restored(&mut self) {
        if self.handle.is_none()
            && let Err(e) = self.refresh()
        {
            warn!(node = ?self.id, r#type = %self.type_name, "Could not read the node's properties: {e}");
        }
    }

    fn ensure_live(&mut self) {
        if self.failed {
            return;
        }
        // Creates the resource when it is missing or belongs to a dead instance (a trap is handled by `call`)
        let _ = self.call(None, |_, _, _| Ok(()));
    }
}

impl Drop for WasmNode {
    fn drop(&mut self) {
        let Some((generation, resource)) = self.handle else { return };
        let mut rt = lock(&self.runtime);
        // The resource lives only in its own instance; do not instantiate a new one just to drop it.
        if rt.generation == generation && let Some((store, _)) = rt.instance.as_mut() {
            let _ = resource.resource_drop(store);
        }
    }
}

// --- Loading ---

/// Loaded plugin: known to the engine, but its types are registered only while enabled.
pub(crate) struct PluginSlot {
    manifest: PluginManifest,
    definitions: Vec<(NodeDefinition, bool)>,
    types: Vec<TypeDefinition>,
    runtime: Arc<Mutex<Runtime>>,
    active: bool,
    /// Result of the last `configure` call (settings error).
    settings_error: Option<UiMessage>,
    /// Validated translation resources of the plugin: `(locale, Fluent source)`.
    translations: Vec<(String, String)>,
}

/// Graph changes after enabling or disabling a plugin.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PluginChange {
    /// Nodes turned into stubs (on disable) or restored to real nodes (on enable).
    pub replaced: Vec<NodeId>,
    /// Connections removed after restoring nodes (when pins changed).
    pub removed: RemovedConnections,
}

/// Effective settings for a plugin: stored values, falling back to manifest defaults for missing ones.
/// An empty string is treated as unset.
pub fn effective_settings(manifest: &PluginManifest, stored: &[(String, DataValue)]) -> Vec<(String, DataValue)> {
    manifest.settings.iter().filter_map(|def| {
        let value = stored.iter().find(|(k, v)| *k == def.key && !matches!(v, DataValue::String(s) if s.is_empty())).map(|(_, v)| v.clone());
        Some((def.key.clone(), value.or_else(|| def.default.clone())?))
    }).collect()
}

/// First required setting that has no value.
pub fn missing_required(manifest: &PluginManifest, effective: &[(String, DataValue)]) -> Option<UiMessage> {
    manifest.settings.iter()
        .find(|d| d.required && !effective.iter().any(|(k, _)| *k == d.key))
        .map(|d| UiMessage::new("error-setting-required", format!("Required setting \"{}\" is not set", d.title)).arg("setting", &d.title).arg("plugin", &manifest.name).arg("key", &d.key))
}

/// A plugin crash reported to the user.
fn crash_message(reason: &str) -> UiMessage {
    UiMessage::new("error-plugin-crash", format!("Plugin crashed: {reason}")).arg("reason", reason)
}

fn plugin_not_loaded(name: &str) -> UiMessage {
    UiMessage::new("error-plugin-not-loaded", format!("Plugin {name} is not loaded")).arg("plugin", name)
}

/// Validates the translations a plugin ships: normalizes locale codes, rejects resources with syntax
/// errors and drops ids outside the plugin's namespace. Problems are logged (in English) and never fatal.
fn validate_translations(plugin: &str, raw: Vec<(String, String)>, allowed: &AllowedIds) -> Vec<(String, String)> {
    let mut valid = Vec::new();
    for (locale, source) in raw {
        let Some(code) = normalize_locale(&locale) else {
            warn!(plugin, locale = %locale, "Ignoring translations with an invalid locale code");
            continue;
        };
        match filter_plugin_resource(plugin, &code, &source, allowed) {
            Ok(filtered) => valid.push((code, filtered)),
            Err(e) => warn!(plugin, locale = %code, "Rejected a translation resource: {e}"),
        }
    }
    valid
}

fn named_types(t: &DataType, out: &mut Vec<String>) {
    match t {
        DataType::Named(name) => out.push(name.clone()),
        DataType::List(inner) => named_types(inner, out),
        _ => {}
    }
}

impl TickEngine {
    /// Loads a plugin component and registers its node types (`add_plugin` + `enable_plugin` with
    /// default settings).
    ///
    /// `grant` decides which capabilities to grant after reading the manifest. The `Storage` capability
    /// gives the plugin the `storage_root/<name>` directory; without `storage_root` it is ignored.
    pub fn load_plugin(
        &mut self,
        wasm: &[u8],
        storage_root: Option<&Path>,
        grant: impl FnOnce(&PluginManifest) -> Vec<Capability>,
    ) -> Result<PluginManifest, UiMessage> {
        let manifest = self.add_plugin(wasm, storage_root, grant)?;
        let settings = effective_settings(&manifest, &[]);
        if let Err(e) = self.enable_plugin(&manifest.name, &settings) {
            self.plugins.remove(&manifest.name);
            return Err(e);
        }
        Ok(manifest)
    }

    /// Compiles a plugin and reads its manifest without enabling it: node types are not registered yet.
    pub fn add_plugin(
        &mut self,
        wasm: &[u8],
        storage_root: Option<&Path>,
        grant: impl FnOnce(&PluginManifest) -> Vec<Capability>,
    ) -> Result<PluginManifest, UiMessage> {
        let component = Component::new(engine(), wasm).map_err(|e| UiMessage::plain(format!("Invalid component: {e:#}")))?;
        let mut linker = Linker::new(engine());
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| UiMessage::plain(e.to_string()))?;
        bindings::Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(|e| UiMessage::plain(e.to_string()))?;
        let pre = linker.instantiate_pre(&component)
            .and_then(bindings::PluginPre::new)
            .map_err(|e| UiMessage::plain(format!("The plugin is incompatible with the host: {e:#}")))?;

        // Manifest and definitions are read from a temporary unprivileged instance.
        let mut runtime = Runtime {
            pre, grants: Arc::default(), allowed: AllowedIds::default(), instance: None, generation: 0, settings: None, configure_error: None, tokio: None, tick_backoff_until: 0,
        };
        let (manifest, definitions, types, translations) = runtime.api()
            .and_then(|(store, api)| Ok((
                api.call_manifest(&mut *store)?,
                api.call_definitions(&mut *store)?,
                api.call_types(&mut *store)?,
                api.call_translations(&mut *store)?,
            )))
            .map_err(|e| UiMessage::plain(format!("Plugin initialization failed: {e:#}")))?;
        runtime.instance = None;

        let manifest = from_wit_manifest(manifest);
        if manifest.name.is_empty() || !manifest.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(UiMessage::plain(format!("Invalid plugin name: {:?}", manifest.name)));
        }
        if self.plugins.contains_key(&manifest.name) {
            return Err(UiMessage::plain(format!("Plugin {} is already loaded", manifest.name)));
        }
        let definitions: Vec<(NodeDefinition, bool)> = definitions.into_iter().map(from_wit_definition).collect();
        let types: Vec<TypeDefinition> = types.into_iter().map(from_wit_type_definition).collect();
        for (def, _) in &definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(UiMessage::plain(format!("Node type {} is already registered", def.type_name)));
            }
        }

        let mut grants = Grants { name: manifest.name.clone(), ..Grants::default() };
        for capability in grant(&manifest) {
            match capability {
                Capability::Http { allowed_domains } => grants.http_domains.extend(allowed_domains),
                Capability::WebSocket { allowed_hosts } => grants.ws_hosts.extend(allowed_hosts),
                Capability::Storage => {
                    if let Some(root) = storage_root {
                        let dir = root.join(&manifest.name);
                        std::fs::create_dir_all(&dir).map_err(|e| UiMessage::plain(format!("Plugin directory {}: {e}", dir.display())))?;
                        grants.storage_dir = Some(dir);
                    }
                }
            }
        }
        runtime.grants = Arc::new(grants);

        let node_types: Vec<String> = definitions.iter().map(|(d, _)| d.type_name.clone()).collect();
        let type_names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();
        let allowed = AllowedIds::new(&manifest.name, &node_types, &type_names);
        runtime.allowed = allowed.clone();
        let translations = validate_translations(&manifest.name, translations, &allowed);

        self.plugins.insert(manifest.name.clone(), PluginSlot {
            manifest: manifest.clone(),
            definitions,
            types,
            runtime: Arc::new(Mutex::new(runtime)),
            active: false,
            settings_error: None,
            translations,
        });
        Ok(manifest)
    }

    /// Calls `plugin-tick` of every enabled plugin and applies the returned emissions. Runs once per tick,
    /// after finished tasks are delivered and before event sources are polled.
    pub(crate) fn tick_plugins(&mut self, next_tick_queue: &mut std::collections::VecDeque<(NodeId, PinId)>) {
        let active: Vec<(String, Arc<Mutex<Runtime>>)> = self.plugins.iter()
            .filter(|(_, p)| p.active)
            .map(|(name, p)| (name.clone(), p.runtime.clone()))
            .collect();
        for (name, runtime) in active {
            let result = lock(&runtime).plugin_tick(&mut self.host_state);
            match result {
                Ok(emissions) => self.apply_emissions(&name, emissions, next_tick_queue),
                Err(reason) => self.host_state.log(NodeId(uuid::Uuid::nil()), format!("[ERROR] Plugin {name} crashed in plugin-tick: {reason}")),
            }
        }
    }

    /// Applies emissions as if the target nodes had executed. An emission for a node that does not exist,
    /// belongs to another plugin (or is a stub) is dropped, as is every invalid pin or value; each drop logs an error.
    fn apply_emissions(&mut self, plugin: &str, emissions: Vec<wit::Emission>, next_tick_queue: &mut std::collections::VecDeque<(NodeId, PinId)>) {
        for emission in emissions {
            let id = NodeId(uuid::Uuid::from_u64_pair(emission.node.0, emission.node.1));
            let definition = self.nodes.get(&id).map(|n| n.definition());
            let Some(definition) = definition.filter(|d| self.registry.owner_of(&d.type_name) == Some(plugin)) else {
                self.host_state.log(id, format!("[ERROR] Plugin {plugin} emitted outputs for node {id:?}, which is not one of its nodes: dropped"));
                continue;
            };
            let (outputs, errors) = validate_emission_outputs(&definition, emission.outputs);
            for e in errors {
                self.host_state.log(id, format!("[ERROR] {e}"));
            }
            if !outputs.is_empty() {
                self.apply_outputs(id, Vec::new(), outputs, next_tick_queue);
            }
        }
    }

    /// Translation resources of the core and of every enabled plugin, merged per locale
    /// (core first, then plugins in name order; the first definition of an id wins).
    pub fn translations(&self) -> Translations {
        let plugin_sources = self.plugins.values().filter(|p| p.active).flat_map(|p| p.translations.iter());
        crate::translations::merge_resources(
            crate::translations::CORE_TRANSLATIONS.iter().copied()
                .chain(plugin_sources.map(|(l, s)| (l.as_str(), s.as_str()))),
        )
    }

    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn plugin_manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name).map(|p| &p.manifest)
    }

    pub fn plugin_active(&self, name: &str) -> bool {
        self.plugins.get(name).is_some_and(|p| p.active)
    }

    /// Error from the last `configure` call ("settings error"); the plugin remains loaded.
    pub fn plugin_settings_error(&self, name: &str) -> Option<UiMessage> {
        self.plugins.get(name)?.settings_error.clone()
    }

    /// Enables a plugin: registers its types, applies settings (`settings` already includes defaults,
    /// see `effective_settings`), and revives stubs of its types into real nodes.
    pub fn enable_plugin(&mut self, name: &str, settings: &[(String, DataValue)]) -> Result<PluginChange, UiMessage> {
        let slot = self.plugins.get(name).ok_or_else(|| plugin_not_loaded(name))?;
        if slot.active {
            return Err(UiMessage::plain(format!("Plugin {name} is already enabled")));
        }
        let (definitions, types, runtime) = (slot.definitions.clone(), slot.types.clone(), slot.runtime.clone());
        for (def, _) in &definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(UiMessage::plain(format!("Node type {} is already registered", def.type_name)));
            }
        }
        let node_definitions: Vec<NodeDefinition> = definitions.iter().map(|(d, _)| d.clone()).collect();
        self.register_types_owned(Some(name), &types, &node_definitions).map_err(|e| UiMessage::plain(format!("Plugin {name}: {e}")))?;
        for (def, polls) in definitions {
            let runtime = runtime.clone();
            self.registry.register_owned(name, Box::new(move |id| Box::new(WasmNode::new(id, def.clone(), polls, runtime.clone()))));
        }

        let settings_error = self.apply_settings(name, settings);
        let slot = self.plugins.get_mut(name).expect("the plugin was just found");
        slot.active = true;
        slot.settings_error = settings_error;

        let (replaced, removed) = self.revive_stubs();
        Ok(PluginChange { replaced, removed })
    }

    /// Disables a plugin: its nodes become stubs (connections and properties are preserved), node
    /// and data types are unregistered, and the plugin instance is dropped. Rejected if another
    /// enabled plugin uses this plugin's data types.
    pub fn disable_plugin(&mut self, name: &str) -> Result<PluginChange, UiMessage> {
        let slot = self.plugins.get(name).ok_or_else(|| plugin_not_loaded(name))?;
        if !slot.active {
            return Err(UiMessage::plain(format!("Plugin {name} is already disabled")));
        }
        let owned: Vec<&String> = self.type_owners.iter().filter(|(_, o)| *o == name).map(|(t, _)| t).collect();
        let dependents: Vec<&str> = self.plugins.iter()
            .filter(|(other, p)| other.as_str() != name && p.active)
            .filter(|(_, p)| {
                let mut used = Vec::new();
                for (def, _) in &p.definitions {
                    for pin in &def.pins {
                        if let PinType::Data(t) = &pin.pin_type {
                            named_types(t, &mut used);
                        }
                    }
                }
                for t in &p.types {
                    for f in t.fields() {
                        named_types(&f.data_type, &mut used);
                    }
                }
                used.iter().any(|u| owned.contains(&u))
            })
            .map(|(other, _)| other.as_str())
            .collect();
        if !dependents.is_empty() {
            let list = dependents.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(", ");
            return Err(UiMessage::new(
                "error-plugin-disable-blocked",
                format!("Plugin \"{name}\" cannot be disabled: its data types are used by {list}; disable {} first", if dependents.len() == 1 { "it" } else { "them" }),
            ).arg("plugin", name).arg("dependents", list));
        }

        let node_types: std::collections::HashSet<String> = self.registry.types_owned_by(name).into_iter().collect();
        let replaced = self.replace_with_stubs(&node_types, name);
        self.registry.unregister_owner(name);
        let type_names: Vec<String> = self.type_owners.iter().filter(|(_, o)| *o == name).map(|(t, _)| t.clone()).collect();
        for t in type_names {
            self.type_owners.remove(&t);
            self.types.remove(&t);
        }
        let slot = self.plugins.get_mut(name).expect("the plugin was just found");
        slot.active = false;
        slot.settings_error = None;
        // Node resources were already dropped when replaced with stubs; the instance is no longer needed
        lock(&slot.runtime).discard();
        Ok(PluginChange { replaced, removed: RemovedConnections::default() })
    }

    /// Passes new settings to the plugin (`settings` already includes defaults). Does nothing for a
    /// disabled plugin: settings will be applied when enabled. `Err` is the plugin's response, which
    /// is also recorded as the settings error.
    pub fn configure_plugin(&mut self, name: &str, settings: &[(String, DataValue)]) -> Result<(), UiMessage> {
        let slot = self.plugins.get(name).ok_or_else(|| plugin_not_loaded(name))?;
        if !slot.active {
            return Ok(());
        }
        let error = self.apply_settings(name, settings);
        let slot = self.plugins.get_mut(name).expect("the plugin was just found");
        slot.settings_error = error.clone();
        error.map_or(Ok(()), Err)
    }

    fn apply_settings(&self, name: &str, settings: &[(String, DataValue)]) -> Option<UiMessage> {
        let slot = &self.plugins[name];
        lock(&slot.runtime).configure(settings, self.host_state.runtime.clone()).err().or_else(|| missing_required(&slot.manifest, settings))
    }
}

// --- WIT ↔ ticknode-shared conversion ---

fn encode_value(value: &DataValue) -> wit::Value {
    postcard::to_allocvec(value).expect("DataValue always serializes")
}

fn decode_value(bytes: &[u8]) -> Option<DataValue> {
    postcard::from_bytes(bytes).inspect_err(|e| warn!("The plugin returned an invalid value: {e}")).ok()
}

fn from_wit_data_type(t: wit::DataType) -> DataType {
    let mut ty = match t.base {
        wit::ScalarType::Bool => DataType::Bool,
        wit::ScalarType::Number => DataType::Number,
        wit::ScalarType::String => DataType::String,
        wit::ScalarType::Time => DataType::Time,
        wit::ScalarType::Datetime => DataType::DateTime,
        wit::ScalarType::Duration => DataType::Duration,
        wit::ScalarType::Color => DataType::Color,
        wit::ScalarType::Object => DataType::Object,
        wit::ScalarType::Any => DataType::Any,
        wit::ScalarType::Named(name) => DataType::Named(name),
    };
    for _ in 0..t.list_depth {
        ty = DataType::List(Box::new(ty));
    }
    ty
}

fn from_wit_type_definition(t: wit::TypeDefinition) -> TypeDefinition {
    TypeDefinition {
        name: t.name,
        title: t.title,
        description: t.description,
        color: t.color,
        kind: match t.kind {
            wit::TypeKind::Structure(fields) => TypeKind::Struct {
                fields: fields.into_iter().map(|f| FieldDefinition {
                    name: f.name,
                    title: f.title,
                    description: f.description,
                    data_type: from_wit_data_type(f.data_type),
                    optional: f.optional,
                }).collect(),
            },
            wit::TypeKind::Enumeration(values) => TypeKind::Enum {
                values: values.into_iter().map(|v| EnumValue { value: v.value, title: v.title }).collect(),
            },
        },
    }
}

/// Definition and `polls` flag (not part of `NodeDefinition`: it concerns the host, not the editor).
fn from_wit_definition(d: wit::NodeDefinition) -> (NodeDefinition, bool) {
    let definition = NodeDefinition {
        type_name: d.type_name,
        title: d.title,
        category: d.category,
        description: d.description,
        help: d.help,
        pins: d.pins.into_iter().map(|p| PinDefinition {
            id: PinId(p.id),
            required: p.required,
            name: p.name,
            pin_type: match p.pin_type {
                wit::PinType::Exec => PinType::Exec,
                wit::PinType::Data(t) => PinType::Data(from_wit_data_type(t)),
            },
            direction: match p.direction {
                wit::PinDirection::Input => PinDirection::Input,
                wit::PinDirection::Output => PinDirection::Output,
            } }).collect(),
    };
    (definition, d.polls)
}

fn from_wit_manifest(m: wit::PluginManifest) -> PluginManifest {
    PluginManifest {
        name: m.name,
        version: m.version,
        author: m.author,
        requested_capabilities: m.capabilities.into_iter().map(|c| match c {
            wit::Capability::Http(allowed_domains) => Capability::Http { allowed_domains },
            wit::Capability::Storage => Capability::Storage,
            wit::Capability::Websocket(allowed_hosts) => Capability::WebSocket { allowed_hosts },
        }).collect(),
        settings: m.settings.into_iter().map(|s| SettingDefinition {
            key: s.key,
            title: s.title,
            description: s.description,
            kind: match s.kind {
                wit::SettingKind::Text => SettingKind::Text,
                wit::SettingKind::Number => SettingKind::Number,
                wit::SettingKind::Bool => SettingKind::Bool,
                wit::SettingKind::Secret => SettingKind::Secret,
                wit::SettingKind::Url => SettingKind::Url,
                wit::SettingKind::Select(options) => SettingKind::Select(options),
            },
            required: s.required,
            default: s.default.as_deref().and_then(decode_value),
        }).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_matching() {
        assert!(domain_allowed("example.com", "example.com"));
        assert!(domain_allowed("api.example.com", "example.com"));
        assert!(!domain_allowed("evilexample.com", "example.com"));
        assert!(domain_allowed("anything.net", "*"));
    }

    fn consumer_slot(runtime: Arc<Mutex<Runtime>>, pins: Vec<PinDefinition>, types: Vec<TypeDefinition>) -> PluginSlot {
        let definition = NodeDefinition {
            type_name: "consumer/node".into(),
            title: "Consumer".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            pins,
        };
        PluginSlot {
            manifest: PluginManifest::new("consumer", "0.1.0", "test"),
            definitions: vec![(definition, false)],
            types,
            runtime,
            active: true,
            settings_error: None,
            translations: Vec::new(),
        }
    }

    /// Disabling a plugin whose types are used by another enabled plugin (in pins or type fields)
    /// is rejected. The consumer is a mock: it shares its runtime with test_plugin because the check
    /// inspects only definitions.
    #[test]
    fn disabling_is_rejected_while_another_plugin_uses_its_types() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
        let Ok(bytes) = std::fs::read(&path) else {
            assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} not found", path.display());
            return;
        };
        let mut engine = TickEngine::new();
        engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
        engine.enable_plugin("test_plugin", &[]).unwrap();
        let runtime = engine.plugins["test_plugin"].runtime.clone();
        let point = DataType::Named("test.point".into());

        let pin = PinDefinition { id: PinId(0), name: "P".into(), pin_type: PinType::Data(DataType::List(Box::new(point.clone()))), direction: PinDirection::Input, required: false };
        engine.plugins.insert("consumer".into(), consumer_slot(runtime.clone(), vec![pin], vec![]));
        let err = engine.disable_plugin("test_plugin").unwrap_err();
        let err = err.to_string();
        assert!(err.contains("consumer") && err.contains("test_plugin"), "{err}");
        assert!(engine.plugin_active("test_plugin") && engine.types.get("test.point").is_some());

        let field = TypeDefinition::new_struct("consumer.wrap", "Wrap", vec![FieldDefinition::new("p", "P", point)]);
        engine.plugins.insert("consumer".into(), consumer_slot(runtime.clone(), vec![], vec![field]));
        assert!(engine.disable_plugin("test_plugin").is_err());

        // Consumer disabled — no dependency left
        engine.plugins.get_mut("consumer").unwrap().active = false;
        engine.disable_plugin("test_plugin").unwrap();
        assert!(engine.types.get("test.point").is_none());
    }

    fn data_emission(node: NodeId, pin: u32, value: DataValue) -> wit::Emission {
        wit::Emission { node: node.0.as_u64_pair(), outputs: vec![wit::Output::Data((pin, encode_value(&value)))] }
    }

    /// Emissions are applied only to existing nodes of the emitting plugin: a built-in node, a node of
    /// another plugin and an unknown id are dropped with an error log; invalid pins are dropped too.
    #[test]
    fn emissions_for_foreign_nodes_are_dropped() {
        let mut engine = TickEngine::new();
        let value = DataValue::Number(NumberValue::Int(7));
        // A built-in node (no owner) and a `const` node whose type is registered as owned by plugin "mine"
        let builtin = NodeId::generate();
        engine.create_node(builtin, "test/debug", &[]).unwrap();
        engine.registry.register_owned("mine", Box::new(|id| Box::new(crate::ConstNode { id, value: DataValue::Number(NumberValue::Int(0)) })));
        let mine = NodeId::generate();
        engine.create_node(mine, "const", &[]).unwrap();
        let mut queue = std::collections::VecDeque::new();

        // Rejected for "other" (foreign plugin) and for "mine" when the node is built-in
        engine.apply_emissions("mine", vec![data_emission(builtin, 0, value.clone())], &mut queue);
        assert!(engine.data_cache.is_empty());
        engine.apply_emissions("other", vec![data_emission(mine, 0, value.clone())], &mut queue);
        assert!(engine.data_cache.is_empty());
        assert!(engine.host_state.logs.iter().any(|(n, m)| *n == mine && m.contains("not one of its nodes")), "{:?}", engine.host_state.logs);

        // Unknown node
        let ghost = NodeId::generate();
        engine.apply_emissions("mine", vec![data_emission(ghost, 0, value.clone())], &mut queue);
        assert!(!engine.data_cache.contains_key(&ghost));
        assert!(engine.host_state.logs.iter().any(|(n, _)| *n == ghost));

        // Invalid pins: a data value on a nonexistent pin and an exec pulse on a data pin
        engine.host_state.logs.clear();
        let bad = wit::Emission { node: mine.0.as_u64_pair(), outputs: vec![wit::Output::Exec(0), wit::Output::Data((99, encode_value(&value)))] };
        engine.apply_emissions("mine", vec![bad], &mut queue);
        assert_eq!(engine.host_state.logs.len(), 2, "{:?}", engine.host_state.logs);
        assert!(engine.data_cache.is_empty());

        // Valid emission is applied and traced like an execution
        engine.host_state.logs.clear();
        engine.apply_emissions("mine", vec![data_emission(mine, 0, value.clone())], &mut queue);
        assert_eq!(engine.data_cache[&mine][&PinId(0)], value);
        assert!(engine.executed.iter().any(|t| t.node_id == mine));
        assert!(engine.host_state.logs.is_empty());
    }

    #[test]
    fn list_depth_becomes_nested_lists() {
        let t = from_wit_data_type(wit::DataType { base: wit::ScalarType::Number, list_depth: 2 });
        assert_eq!(t, DataType::List(Box::new(DataType::List(Box::new(DataType::Number)))));
    }
}
