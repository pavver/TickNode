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

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.consume_fuel(true);
        Engine::new(&config).expect("конфігурація wasmtime коректна")
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
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl PluginState {
    fn call(&mut self) -> Result<&mut CallCtx, String> {
        self.call.as_mut().ok_or_else(|| "Доступно лише під час execute/poll".to_string())
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
        let url = url::Url::parse(&request.url).map_err(|e| format!("Некоректний URL: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!("Непідтримувана схема: {}", url.scheme()));
        }
        let host = url.host_str().unwrap_or_default();
        if !self.grants.http_domains.iter().any(|d| domain_allowed(host, d)) {
            return Err(format!("Плагін не має права на HTTP до {host}"));
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
}

/// `example.com` allows the domain itself and its subdomains, but not `evilexample.com`.
fn domain_allowed(host: &str, allowed: &str) -> bool {
    allowed == "*" || host == allowed || host.ends_with(&format!(".{}", allowed))
}

// --- Plugin instance ---

struct Runtime {
    pre: bindings::PluginPre<PluginState>,
    grants: Arc<Grants>,
    /// `None` after a trap: a new instance is created on the next call.
    instance: Option<(Store<PluginState>, bindings::Plugin)>,
    /// Incremented with each new instance; resources from older instances are invalid.
    generation: u64,
    /// Settings to apply to each new instance (`configure`) before recreating nodes.
    settings: Option<Vec<wit::Property>>,
    /// Result of the last `configure` called during instance creation.
    configure_error: Option<String>,
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
            };
            let mut store = Store::new(engine(), state);
            store.limiter(|s| &mut s.limits);
            store.set_fuel(FUEL_PER_CALL)?;
            let plugin = self.pre.instantiate(&mut store)?;
            // A recreated instance after a crash receives settings first — otherwise it would lose its config
            if let Some(settings) = &self.settings {
                store.set_fuel(FUEL_PER_CALL)?;
                self.configure_error = plugin.ticknode_plugin_nodes().call_configure(&mut store, settings)?.err();
            }
            self.instance = Some((store, plugin));
            self.generation += 1;
        }
        Ok(self.generation)
    }

    fn api(&mut self) -> wasmtime::Result<(&mut Store<PluginState>, &NodesApi)> {
        self.instantiate()?;
        let (store, plugin) = self.instance.as_mut().expect("екземпляр щойно створено");
        store.set_fuel(FUEL_PER_CALL)?;
        Ok((store, plugin.ticknode_plugin_nodes()))
    }

    /// Stores settings and applies them: to an existing instance via `configure`,
    /// while a new instance receives them upon creation. `Err` is the plugin's response (or a trap).
    fn configure(&mut self, settings: &[(String, DataValue)]) -> Result<(), String> {
        let props: Vec<wit::Property> = settings.iter()
            .map(|(name, value)| wit::Property { name: name.clone(), value: encode_value(value) })
            .collect();
        self.settings = Some(props.clone());
        let fresh = self.instance.is_none();
        self.instantiate().map_err(|e| format!("Аварія плагіна: {}", short_error(&e)))?;
        if fresh {
            return self.configure_error.clone().map_or(Ok(()), Err);
        }
        let outcome = match self.api() {
            Ok((store, api)) => api.call_configure(store, &props),
            Err(e) => Err(e),
        };
        match outcome {
            Ok(result) => {
                self.configure_error = result.clone().err();
                result
            }
            Err(e) => {
                warn!("Аварія плагіна під час configure: {e:?}");
                let reason = short_error(&e);
                self.discard();
                Err(format!("Аварія плагіна: {reason}"))
            }
        }
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
    warn!("Аварія плагіна: {e:?}");
    CallError::Trap(short_error(&e))
}

fn short_error(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<wasmtime::Trap>() {
        Some(wasmtime::Trap::OutOfFuel) => "перевищено ліміт обчислень за один виклик (нескінченний цикл?)".to_string(),
        Some(wasmtime::Trap::UnreachableCodeReached) => "паніка в плагіні (повідомлення — у лозі сервера)".to_string(),
        Some(trap) => trap.to_string(),
        None => e.root_cause().to_string(),
    }
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Trap(e) => write!(f, "Аварія плагіна: {e}"),
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
            return Err(CallError::Rejected("Нода зупинена після помилки плагіна; змініть властивість, щоб перезапустити".into()));
        }
        let runtime = self.runtime.clone();
        let mut rt = lock(&runtime);
        let generation = rt.instantiate().map_err(trap_error)?;
        let (store, api) = rt.api().map_err(trap_error)?;

        let resource = match self.handle {
            Some((g, r)) if g == generation => Ok(r),
            _ => create_resource(store, api, &self.type_name, &self.properties),
        };
        let result = match resource {
            Ok(resource) => {
                self.handle = Some((generation, resource));
                if let Some(h) = &mut host {
                    store.data_mut().call = Some(CallCtx { node_id: self.id, host: std::mem::take(*h) });
                }
                let result = f(store, api, resource);
                if let Some(h) = host {
                    *h = store.data_mut().call.take().expect("контекст виклику на місці").host;
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

    /// A value emitted on a custom struct output pin must carry the tag of that exact type: a plugin
    /// cannot emit another type or an untyped object disguised as `Named`.
    fn check_output_type(&self, pin: PinId, value: &DataValue) -> Result<(), String> {
        let declared = self.definition.pins.iter()
            .find(|p| p.id == pin && p.direction == PinDirection::Output)
            .and_then(|p| match &p.pin_type {
                PinType::Data(DataType::Named(name)) => Some(name),
                _ => None,
            });
        match (declared, value) {
            (Some(name), DataValue::Object { type_name, .. }) if type_name.as_deref() != Some(name) => Err(format!(
                "Плагін видав на вихід типу {name} значення {}: значення відкинуто",
                type_name.as_deref().map_or("без типу".to_string(), |t| format!("типу {t}")),
            )),
            _ => Ok(()),
        }
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
                    match self.check_output_type(PinId(pin), &value) {
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

fn create_resource(
    store: &mut Store<PluginState>,
    api: &NodesApi,
    type_name: &str,
    properties: &[(String, DataValue)],
) -> Result<ResourceAny, CallError> {
    let node = api.call_create_node(&mut *store, type_name).map_err(trap_error)?.map_err(CallError::Rejected)?;
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
            warn!(node = ?self.id, r#type = %self.type_name, "Плагін не прийняв властивість {name}: {e}");
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> { self.properties.clone() }
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
    settings_error: Option<String>,
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
pub fn missing_required(manifest: &PluginManifest, effective: &[(String, DataValue)]) -> Option<String> {
    manifest.settings.iter()
        .find(|d| d.required && !effective.iter().any(|(k, _)| *k == d.key))
        .map(|d| format!("Не задано обов'язкове налаштування «{}»", d.title))
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
    ) -> Result<PluginManifest, String> {
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
    ) -> Result<PluginManifest, String> {
        let component = Component::new(engine(), wasm).map_err(|e| format!("Некоректний компонент: {e:#}"))?;
        let mut linker = Linker::new(engine());
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|e| e.to_string())?;
        bindings::Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s).map_err(|e| e.to_string())?;
        let pre = linker.instantiate_pre(&component)
            .and_then(bindings::PluginPre::new)
            .map_err(|e| format!("Плагін несумісний з хостом: {e:#}"))?;

        // Manifest and definitions are read from a temporary unprivileged instance.
        let mut runtime = Runtime { pre, grants: Arc::default(), instance: None, generation: 0, settings: None, configure_error: None };
        let (manifest, definitions, types) = runtime.api()
            .and_then(|(store, api)| Ok((api.call_manifest(&mut *store)?, api.call_definitions(&mut *store)?, api.call_types(&mut *store)?)))
            .map_err(|e| format!("Помилка ініціалізації плагіна: {e:#}"))?;
        runtime.instance = None;

        let manifest = from_wit_manifest(manifest);
        if manifest.name.is_empty() || !manifest.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("Некоректна назва плагіна: {:?}", manifest.name));
        }
        if self.plugins.contains_key(&manifest.name) {
            return Err(format!("Плагін {} уже завантажено", manifest.name));
        }
        let definitions: Vec<(NodeDefinition, bool)> = definitions.into_iter().map(from_wit_definition).collect();
        let types: Vec<TypeDefinition> = types.into_iter().map(from_wit_type_definition).collect();
        for (def, _) in &definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(format!("Тип ноди {} уже зареєстровано", def.type_name));
            }
        }

        let mut grants = Grants { name: manifest.name.clone(), ..Grants::default() };
        for capability in grant(&manifest) {
            match capability {
                Capability::Http { allowed_domains } => grants.http_domains.extend(allowed_domains),
                Capability::Storage => {
                    if let Some(root) = storage_root {
                        let dir = root.join(&manifest.name);
                        std::fs::create_dir_all(&dir).map_err(|e| format!("Каталог плагіна {}: {e}", dir.display()))?;
                        grants.storage_dir = Some(dir);
                    }
                }
            }
        }
        runtime.grants = Arc::new(grants);

        self.plugins.insert(manifest.name.clone(), PluginSlot {
            manifest: manifest.clone(),
            definitions,
            types,
            runtime: Arc::new(Mutex::new(runtime)),
            active: false,
            settings_error: None,
        });
        Ok(manifest)
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
    pub fn plugin_settings_error(&self, name: &str) -> Option<String> {
        self.plugins.get(name)?.settings_error.clone()
    }

    /// Enables a plugin: registers its types, applies settings (`settings` already includes defaults,
    /// see `effective_settings`), and revives stubs of its types into real nodes.
    pub fn enable_plugin(&mut self, name: &str, settings: &[(String, DataValue)]) -> Result<PluginChange, String> {
        let slot = self.plugins.get(name).ok_or_else(|| format!("Плагін {name} не завантажено"))?;
        if slot.active {
            return Err(format!("Плагін {name} уже увімкнено"));
        }
        let (definitions, types, runtime) = (slot.definitions.clone(), slot.types.clone(), slot.runtime.clone());
        for (def, _) in &definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(format!("Тип ноди {} уже зареєстровано", def.type_name));
            }
        }
        let node_definitions: Vec<NodeDefinition> = definitions.iter().map(|(d, _)| d.clone()).collect();
        self.register_types_owned(Some(name), &types, &node_definitions).map_err(|e| format!("Плагін {name}: {e}"))?;
        for (def, polls) in definitions {
            let runtime = runtime.clone();
            self.registry.register_owned(name, Box::new(move |id| Box::new(WasmNode::new(id, def.clone(), polls, runtime.clone()))));
        }

        let settings_error = Self::apply_settings(&self.plugins[name].manifest, &runtime, settings);
        let slot = self.plugins.get_mut(name).expect("плагін щойно знайдено");
        slot.active = true;
        slot.settings_error = settings_error;

        let (replaced, removed) = self.revive_stubs();
        Ok(PluginChange { replaced, removed })
    }

    /// Disables a plugin: its nodes become stubs (connections and properties are preserved), node
    /// and data types are unregistered, and the plugin instance is dropped. Rejected if another
    /// enabled plugin uses this plugin's data types.
    pub fn disable_plugin(&mut self, name: &str) -> Result<PluginChange, String> {
        let slot = self.plugins.get(name).ok_or_else(|| format!("Плагін {name} не завантажено"))?;
        if !slot.active {
            return Err(format!("Плагін {name} уже вимкнено"));
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
            return Err(format!(
                "Плагін «{name}» не можна вимкнути: його типи даних використовує {} — спершу вимкніть {}",
                dependents.iter().map(|d| format!("«{d}»")).collect::<Vec<_>>().join(", "),
                if dependents.len() == 1 { "його" } else { "їх" },
            ));
        }

        let node_types: std::collections::HashSet<String> = self.registry.types_owned_by(name).into_iter().collect();
        let replaced = self.replace_with_stubs(&node_types, name);
        self.registry.unregister_owner(name);
        let type_names: Vec<String> = self.type_owners.iter().filter(|(_, o)| *o == name).map(|(t, _)| t.clone()).collect();
        for t in type_names {
            self.type_owners.remove(&t);
            self.types.remove(&t);
        }
        let slot = self.plugins.get_mut(name).expect("плагін щойно знайдено");
        slot.active = false;
        slot.settings_error = None;
        // Node resources were already dropped when replaced with stubs; the instance is no longer needed
        lock(&slot.runtime).discard();
        Ok(PluginChange { replaced, removed: RemovedConnections::default() })
    }

    /// Passes new settings to the plugin (`settings` already includes defaults). Does nothing for a
    /// disabled plugin: settings will be applied when enabled. `Err` is the plugin's response, which
    /// is also recorded as the settings error.
    pub fn configure_plugin(&mut self, name: &str, settings: &[(String, DataValue)]) -> Result<(), String> {
        let slot = self.plugins.get(name).ok_or_else(|| format!("Плагін {name} не завантажено"))?;
        if !slot.active {
            return Ok(());
        }
        let error = Self::apply_settings(&slot.manifest, &slot.runtime.clone(), settings);
        let slot = self.plugins.get_mut(name).expect("плагін щойно знайдено");
        slot.settings_error = error.clone();
        error.map_or(Ok(()), Err)
    }

    fn apply_settings(manifest: &PluginManifest, runtime: &Arc<Mutex<Runtime>>, settings: &[(String, DataValue)]) -> Option<String> {
        lock(runtime).configure(settings).err().or_else(|| missing_required(manifest, settings))
    }
}

// --- WIT ↔ ticknode-shared conversion ---

fn encode_value(value: &DataValue) -> wit::Value {
    postcard::to_allocvec(value).expect("DataValue завжди серіалізується")
}

fn decode_value(bytes: &[u8]) -> Option<DataValue> {
    postcard::from_bytes(bytes).inspect_err(|e| warn!("Плагін повернув некоректне значення: {e}")).ok()
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
        }
    }

    /// Disabling a plugin whose types are used by another enabled plugin (in pins or type fields)
    /// is rejected. The consumer is a mock: it shares its runtime with test_plugin because the check
    /// inspects only definitions.
    #[test]
    fn disabling_is_rejected_while_another_plugin_uses_its_types() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
        let Ok(bytes) = std::fs::read(&path) else {
            assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} не знайдено", path.display());
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

    #[test]
    fn list_depth_becomes_nested_lists() {
        let t = from_wit_data_type(wit::DataType { base: wit::ScalarType::Number, list_depth: 2 });
        assert_eq!(t, DataType::List(Box::new(DataType::List(Box::new(DataType::Number)))));
    }
}
