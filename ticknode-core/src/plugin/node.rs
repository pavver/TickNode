use std::sync::{Arc, Mutex};
use ticknode_shared::*;
use tracing::warn;
use wasmtime::component::ResourceAny;
use wasmtime::Store;
use crate::host::{HostState, TaskId, TaskOutput, TaskResult};
use crate::{Node, NodeContext};
use super::{lock, wit, NodesApi};
use super::runtime::{Runtime, short_error};
use super::host::{CallCtx, PluginState};
use super::convert::{encode_value, decode_value, from_wit_definition};
use super::outputs::validate_emission_outputs;

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
    /// Owning plugin (fills `PropertyKind::Dynamic::plugin` in refreshed definitions).
    plugin: String,
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

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Trap(e) => write!(f, "Plugin crashed: {e}"),
            CallError::Rejected(e) => write!(f, "{e}"),
        }
    }
}

impl WasmNode {
    pub(super) fn new(id: NodeId, definition: NodeDefinition, polls: bool, runtime: Arc<Mutex<Runtime>>) -> Self {
        let plugin = lock(&runtime).grants.name.clone();
        Self { id, type_name: definition.type_name.clone(), runtime, handle: None, definition, polls, properties: Vec::new(), failed: false, plugin }
    }

    /// Calls a method on the node resource. If `host` is passed, it is moved into the plugin for the
    /// duration of the call (tasks, logs) and restored afterward.
    fn call<R>(
        &mut self,
        host: Option<&mut HostState>,
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
                call_with_host(store, api, resource, self.id, host, f)
            }
            Err(e) => Err(e),
        };
        if let Err(e) = &result {
            self.fail_call(&mut rt, e);
        }
        result
    }

    fn fail_call(&mut self, rt: &mut Runtime, e: &CallError) {
        self.failed = true;
        if matches!(e, CallError::Trap(_)) {
            rt.discard();
        }
    }

    fn refresh(&mut self) -> Result<(), CallError> {
        let (definition, properties) = self.call(None, |store, api, node| {
            Ok((api.node().call_definition(&mut *store, node)?, api.node().call_properties(&mut *store, node)?))
        })?;
        (self.definition, self.polls) = from_wit_definition(definition, &self.plugin);
        self.properties = properties.into_iter().filter_map(|p| Some((p.name, decode_value(&p.value)?))).collect();
        Ok(())
    }

    fn run(&mut self, trigger: wit::Trigger, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let inputs: Vec<(u32, wit::Value)> = ctx.inputs.iter()
            .filter_map(|(pin, value)| Some((pin.0, encode_value(value.as_ref()?))))
            .collect();
        let result = self.call(Some(ctx.host_state), |store, api, node| api.node().call_execute(store, node, &trigger, &inputs));
        match result {
            Ok(outputs) => {
                let (valid, errors) = validate_emission_outputs(&self.definition, outputs);
                for error in errors {
                    ctx.host_state.log(self.id, format!("[ERROR] {error}"));
                }
                valid
            }
            Err(e) => {
                ctx.host_state.log(self.id, format!("[ERROR] {e}"));
                Vec::new()
            }
        }
    }
}

fn call_with_host<R>(
    store: &mut Store<PluginState>, api: &NodesApi, resource: ResourceAny, node_id: NodeId,
    mut host: Option<&mut HostState>,
    f: impl FnOnce(&mut Store<PluginState>, &NodesApi, ResourceAny) -> wasmtime::Result<R>,
) -> Result<R, CallError> {
    if let Some(h) = &mut host {
        store.data_mut().call = Some(CallCtx { node_id, host: std::mem::take(*h) });
    }
    let result = f(store, api, resource);
    if let Some(h) = host {
        *h = store.data_mut().call.take().expect("the call context is in place").host;
    }
    result.map_err(trap_error)
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

    fn polls(&self) -> bool { self.polls }

    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
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
