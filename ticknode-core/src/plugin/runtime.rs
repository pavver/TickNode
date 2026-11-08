use std::sync::{Arc, Mutex, OnceLock};
use ticknode_shared::*;
use tracing::warn;
use wasmtime::component::ResourceTable;
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};
use wasmtime_wasi::{FsPerms, WasiCtxBuilder};
use crate::host::HostState;
use crate::translations::{AllowedIds, sanitize_ui_message};
use crate::ws::{self, WsHub};
use super::{bindings, wit, NodesApi, OptionStore};
use super::host::{CallCtx, Grants, PluginState};
use super::lifecycle::crash_message;
use super::convert::encode_value;

/// Instruction budget per plugin call (guards against infinite loops).
const FUEL_PER_CALL: u64 = 2_000_000_000;
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
/// Pause before `plugin-tick` is called again after the plugin trapped in it.
const TICK_BACKOFF_TICKS: u64 = 50;

pub(super) fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.consume_fuel(true);
        Engine::new(&config).expect("the wasmtime configuration is valid")
    })
}

pub(super) struct Runtime {
    pub(super) pre: bindings::PluginPre<PluginState>,
    pub(super) grants: Arc<Grants>,
    /// Which Fluent ids the plugin may define; also its UI error key namespace.
    pub(super) allowed: AllowedIds,
    /// `None` after a trap: a new instance is created on the next call.
    pub(super) instance: Option<(Store<PluginState>, bindings::Plugin)>,
    /// Incremented with each new instance; resources from older instances are invalid.
    pub(super) generation: u64,
    /// Settings to apply to each new instance (`configure`) before recreating nodes.
    pub(super) settings: Option<Vec<wit::Property>>,
    /// Result of the last `configure` called during instance creation.
    pub(super) configure_error: Option<UiMessage>,
    /// Runtime for the instance's WebSocket tasks (`HostState::runtime`, refreshed by the engine).
    pub(super) tokio: Option<tokio::runtime::Handle>,
    /// After a trap in `plugin-tick`, the next call is made no earlier than this tick (a plugin that traps
    /// on every tick would otherwise be recreated and reconnected every tick).
    pub(super) tick_backoff_until: u64,
    pub(super) options: Arc<Mutex<OptionStore>>,
}

impl Runtime {
    pub(super) fn instantiate(&mut self) -> wasmtime::Result<u64> {
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
                options: self.options.clone(),
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

    pub(super) fn api(&mut self) -> wasmtime::Result<(&mut Store<PluginState>, &NodesApi)> {
        self.instantiate()?;
        let (store, plugin) = self.instance.as_mut().expect("the instance was just created");
        store.data_mut().ws.set_runtime(self.tokio.clone());
        store.set_fuel(FUEL_PER_CALL)?;
        Ok((store, plugin.ticknode_plugin_nodes()))
    }

    /// Stores settings and applies them: to an existing instance via `configure`,
    /// while a new instance receives them upon creation. `Err` is the plugin's response (or a trap).
    pub(super) fn configure(&mut self, settings: &[(String, DataValue)], tokio: Option<tokio::runtime::Handle>) -> Result<(), UiMessage> {
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
    pub(super) fn plugin_tick(&mut self, host: &mut HostState) -> Result<Vec<wit::Emission>, String> {
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
    pub(super) fn discard(&mut self) {
        self.instance = None;
        self.generation += 1;
    }
}

pub(super) fn short_error(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<wasmtime::Trap>() {
        Some(wasmtime::Trap::OutOfFuel) => "per-call compute limit exceeded (infinite loop?)".to_string(),
        Some(wasmtime::Trap::UnreachableCodeReached) => "panic in the plugin (see the server log for details)".to_string(),
        Some(trap) => trap.to_string(),
        None => e.root_cause().to_string(),
    }
}
