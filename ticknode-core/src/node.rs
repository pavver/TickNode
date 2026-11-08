use std::collections::HashMap;
use ticknode_shared::*;
use crate::{HostState, TaskId, TaskResult};

pub struct NodeContext<'a> {
    pub node_id: NodeId,
    pub inputs: HashMap<PinId, Option<DataValue>>,
    pub variables: &'a HashMap<String, DataValue>,
    pub host_state: &'a mut HostState,
}

impl<'a> NodeContext<'a> {
    pub fn get_number(&self, pin: PinId, default: NumberValue) -> NumberValue {
        match self.inputs.get(&pin) {
            Some(Some(DataValue::Number(v))) => *v,
            _ => default,
        }
    }
    pub fn get_bool(&self, pin: PinId, default: bool) -> bool {
        match self.inputs.get(&pin) {
            Some(Some(DataValue::Bool(v))) => *v,
            _ => default,
        }
    }
    pub fn get_value(&self, pin: PinId) -> Option<DataValue> {
        self.inputs.get(&pin).cloned().flatten()
    }
    pub fn get_var(&self, name: &str) -> Option<DataValue> {
        self.variables.get(name).cloned()
    }
}

pub trait Node: Send + Sync {
    fn id(&self) -> NodeId;
    fn definition(&self) -> NodeDefinition;
    /// Changes whenever this node's definition changes outside set_property/restored (e.g. a plugin node refreshed after a trap)
    fn definition_revision(&self) -> u64 { 0 }
    fn is_pure(&self) -> bool { false }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput>;
    fn set_property(&mut self, _name: &str, _value: DataValue) {}
    /// Properties needed to reconstruct the node (graph persistence, editor).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    /// The node has been restored: properties from `create_node` have been applied. Subsequent
    /// `set_property` calls are live edits rather than saved state restoration.
    fn restored(&mut self) {}
    /// Whether the node is an event source polled (and charged) at the start of every tick.
    fn polls(&self) -> bool { false }
    /// Poll an event source. A returned pin queues its activation for the current tick's budget.
    fn poll(&mut self, _host: &mut HostState) -> Option<PinId> { None }
    /// A task started by the node via `HostState` (delay, HTTP) has completed.
    /// Called on the completion tick, before executing the queue.
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> { Vec::new() }
    /// Called at the start of every tick (before `plugin-tick`): a plugin node makes sure its resource exists in
    /// the current plugin instance, so the plugin knows every live node (and its id) even before the node
    /// has executed, and again after the instance was recreated following a crash.
    fn ensure_live(&mut self) {}
    /// Whether `ensure_live` does anything for this node. The engine calls it only for such nodes, so an idle
    /// tick does not visit every node of the graph.
    fn needs_live(&self) -> bool { false }
    /// Stub node (plugin for this type is unavailable): explanation for the editor. Such a node does not
    /// execute or accept property changes, but preserves pins and properties so the graph can be saved.
    fn unavailable(&self) -> Option<UiMessage> { None }
}
