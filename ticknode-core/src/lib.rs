use std::collections::{BTreeMap, HashMap, VecDeque};
use ticknode_shared::*;
use ticknode_shared::condition::{MatchCase, MatchInput, match_inputs};
use tracing::info;
use thiserror::Error;
use evalexpr::*;

mod host;
mod plugin;
mod custom_types;
mod stub;
pub mod translations;

pub use host::*;
pub use plugin::*;
pub use custom_types::*;
pub use stub::StubNode;

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
    fn is_pure(&self) -> bool { false }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput>;
    fn set_property(&mut self, _name: &str, _value: DataValue) {}
    /// Properties needed to reconstruct the node (graph persistence, editor).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    /// The node has been restored: properties from `create_node` have been applied. Subsequent
    /// `set_property` calls are live edits rather than saved state restoration.
    fn restored(&mut self) {}
    /// Event source nodes (timers, triggers): called at the start of every tick.
    /// A returned pin triggers execution of the node in the same tick.
    fn poll(&mut self, _host: &mut HostState) -> Option<PinId> { None }
    /// A task started by the node via `HostState` (delay, HTTP) has completed.
    /// Called on the completion tick, before executing the queue.
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> { Vec::new() }
    /// Stub node (plugin for this type is unavailable): explanation for the editor. Such a node does not
    /// execute or accept property changes, but preserves pins and properties so the graph can be saved.
    fn unavailable(&self) -> Option<UiMessage> { None }
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum EngineError {
    #[error("Circular reference: node {0:?} already depends on {1:?}")]
    CycleDetected(NodeId, NodeId),
    #[error("Incompatible data types: cannot connect {0:?} to {1:?}")]
    TypeMismatch(DataType, DataType),
    #[error("Invalid pin connection: one or both pins do not exist or have the wrong direction/type")]
    InvalidPinConnection,
    #[error("Node {0:?} not found")]
    NodeNotFound(NodeId),
    #[error("Unknown node type: {0}")]
    UnknownNodeType(String),
    #[error("Node unavailable: {0}; it cannot be changed")]
    PluginUnavailable(UiMessage),
}

impl EngineError {
    /// The error for the user in the editor: a key from the editor's `ui.ftl` (`error-...`) with arguments
    /// and the English text (same as `Display`) as the fallback.
    pub fn ui_message(&self) -> UiMessage {
        let text = self.to_string();
        match self {
            Self::CycleDetected(a, b) => UiMessage::new("error-cycle-detected", text).arg("node", format!("{a:?}")).arg("dependency", format!("{b:?}")),
            Self::TypeMismatch(a, b) => UiMessage::new("error-type-mismatch", text).arg("src", data_type_name(a)).arg("dst", data_type_name(b)),
            Self::InvalidPinConnection => UiMessage::new("error-invalid-pin-connection", text),
            Self::NodeNotFound(id) => UiMessage::new("error-node-not-found", text).arg("node", format!("{id:?}")),
            Self::UnknownNodeType(name) => UiMessage::new("error-unknown-node-type", text).arg("type", name),
            // The stub's own message already says which plugin is missing
            Self::PluginUnavailable(reason) => reason.clone(),
        }
    }
}

pub type NodeCreator = Box<dyn Fn(NodeId) -> Box<dyn Node> + Send + Sync>;

pub struct NodeRegistry {
    definitions: HashMap<String, NodeDefinition>,
    creators: HashMap<String, NodeCreator>,
    /// Which plugin registered the node type (built-in types have no owner).
    owners: HashMap<String, String>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            definitions: HashMap::new(),
            creators: HashMap::new(),
            owners: HashMap::new(),
        };
        registry.register_defaults();
        registry
    }

    /// Registers a node type. The definition is taken from an instance created with default properties,
    /// so the registry and the node itself cannot diverge.
    pub fn register(&mut self, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.register_definition(definition, creator);
    }

    /// Registers a node type on behalf of a plugin (so it can be unregistered when the plugin is disabled).
    pub fn register_owned(&mut self, owner: &str, creator: NodeCreator) {
        let definition = creator(NodeId(uuid::Uuid::nil())).definition();
        self.owners.insert(definition.type_name.clone(), owner.to_string());
        self.register_definition(definition, creator);
    }

    fn register_definition(&mut self, definition: NodeDefinition, creator: NodeCreator) {
        let type_name = definition.type_name.clone();
        self.definitions.insert(type_name.clone(), definition);
        self.creators.insert(type_name, creator);
    }

    pub fn owner_of(&self, type_name: &str) -> Option<&str> {
        self.owners.get(type_name).map(String::as_str)
    }

    pub fn types_owned_by(&self, owner: &str) -> Vec<String> {
        self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect()
    }

    /// Unregisters all node types owned by the plugin and returns their names.
    pub fn unregister_owner(&mut self, owner: &str) -> Vec<String> {
        let names: Vec<String> = self.owners.iter().filter(|(_, o)| *o == owner).map(|(t, _)| t.clone()).collect();
        for name in &names {
            self.owners.remove(name);
            self.definitions.remove(name);
            self.creators.remove(name);
        }
        names
    }

    pub fn get_definitions(&self) -> Vec<NodeDefinition> {
        let mut defs: Vec<_> = self.definitions.values().cloned().collect();
        defs.sort_by(|a, b| (&a.category, &a.title).cmp(&(&b.category, &b.title)));
        defs
    }

    pub fn get_definition(&self, type_name: &str) -> Option<&NodeDefinition> {
        self.definitions.get(type_name)
    }

    pub fn create_node(&self, type_name: &str, id: NodeId) -> Option<Box<dyn Node>> {
        self.creators.get(type_name).map(|f| f(id))
    }

    fn register_defaults(&mut self) {
        self.register(Box::new(|id| Box::new(ClockNode { id, interval_ticks: 10 })));
        self.register(Box::new(|id| Box::new(ConsoleNode { id })));
        self.register(Box::new(|id| Box::new(ConstNode { id, value: DataValue::Number(NumberValue::Int(0)) })));
        for op in [MathOp::Add, MathOp::Sub, MathOp::Mul, MathOp::Div] {
            self.register(Box::new(move |id| Box::new(PureMathNode { id, op })));
        }
        self.register(Box::new(|id| Box::new(MathExpressionNode {
            id,
            expression: "a + b".to_string(),
            variable_mapping: expression_variables("a + b"),
        })));
        for op in [LogOp::And, LogOp::Or, LogOp::Not] {
            self.register(Box::new(move |id| Box::new(LogicalNode { id, op })));
        }
        self.register(Box::new(|id| Box::new(VariableGetNode { id, var_name: "my_var".to_string() })));
        self.register(Box::new(|id| Box::new(VariableSetNode { id, var_name: "my_var".to_string() })));
        self.register(Box::new(|id| Box::new(MatchNode::new(id))));
        self.register(Box::new(|id| Box::new(CompareNode { id })));
        self.register(Box::new(|id| Box::new(ForLoopNode { id, active: false, current_index: 0, count: 0 })));
        self.register(Box::new(|id| Box::new(SleepNode { id })));
        self.register(Box::new(|id| Box::new(ListCreateNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListAddNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListRemoveNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListGetNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ListLengthNode { id, element_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectCreateNode { id })));
        self.register(Box::new(|id| Box::new(ObjectSetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectGetFieldNode { id, field_type: DataType::Number })));
        self.register(Box::new(|id| Box::new(ObjectUnpackNode { id, fields_list: String::new(), parsed_fields: Vec::new() })));
        self.register(Box::new(|id| Box::new(ClickButtonNode { id, label: "Button".to_string(), clicked: false })));
        self.register(Box::new(|id| Box::new(DebugNode { id, prefix: String::new() })));
    }
}

impl Default for NodeRegistry {
    fn default() -> Self { Self::new() }
}

/// Connections removed automatically by the engine (e.g., after a node's pins changed).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RemovedConnections {
    pub exec: Vec<ConnectionModel>,
    pub data: Vec<ConnectionModel>,
}

pub struct TickEngine {
    nodes: HashMap<NodeId, Box<dyn Node>>,
    exec_connections: Vec<ConnectionModel>,
    data_connections: Vec<ConnectionModel>,
    pub registry: NodeRegistry,
    /// Custom data types declared by plugins.
    pub types: TypeRegistry,
    /// Which plugin declared each custom data type.
    type_owners: HashMap<String, String>,
    /// Loaded plugins (both enabled and disabled); see `plugin.rs`.
    plugins: std::collections::BTreeMap<String, plugin::PluginSlot>,
    pub data_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub pure_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub variables: HashMap<String, DataValue>,
    /// Values of unconnected inputs set in the editor (`in:<pin>` properties).
    input_values: HashMap<NodeId, BTreeMap<u32, DataValue>>,
    /// Nodes for which a "missing required input" error has already been logged (to avoid repeating every tick).
    required_reported: std::collections::HashSet<NodeId>,
    pub execution_queue: VecDeque<(NodeId, PinId)>,
    // Shared state for async tasks and handles
    pub host_state: HostState,
    /// Nodes executed during the last tick, with their inputs and outputs (telemetry for the editor).
    /// Pure nodes appear here when evaluated on demand.
    pub executed: Vec<NodeTrace>,
}

impl Default for TickEngine {
    fn default() -> Self { Self::new() }
}

impl TickEngine {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            exec_connections: Vec::new(),
            data_connections: Vec::new(),
            registry: NodeRegistry::new(),
            types: TypeRegistry::new(),
            type_owners: HashMap::new(),
            plugins: Default::default(),
            data_cache: HashMap::new(),
            pure_cache: HashMap::new(),
            variables: HashMap::new(),
            input_values: HashMap::new(),
            required_reported: Default::default(),
            execution_queue: VecDeque::new(),
            host_state: HostState::default(),
            executed: Vec::new(),
        }
    }

    // --- Nodes ---

    pub fn add_node(&mut self, node: Box<dyn Node>) {
        self.nodes.insert(node.id(), node);
    }

    /// Creates a node via the registry and applies its properties. Input values (`in:<pin>`) are returned
    /// separately: they can only be set after properties because pins may depend on them.
    fn build_node(&self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(Box<dyn Node>, Vec<(PinId, DataValue)>), EngineError> {
        let mut node = self.registry.create_node(type_name, id)
            .ok_or_else(|| EngineError::UnknownNodeType(type_name.to_string()))?;
        let mut inputs = Vec::new();
        for (name, value) in properties {
            match parse_input_value_property(name) {
                Some(pin) => inputs.push((pin, value.clone())),
                None => node.set_property(name, value.clone()),
            }
        }
        node.restored();
        Ok((node, inputs))
    }

    /// Creates a node via the registry and applies its properties.
    pub fn create_node(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(), EngineError> {
        let (node, inputs) = self.build_node(id, type_name, properties)?;
        self.add_node(node);
        // Pins depend on properties, so input values are applied afterward
        for (pin, value) in inputs {
            let _ = self.set_input_value(id, pin, Some(value));
        }
        Ok(())
    }

    /// Why the node is unavailable (stub); `None` if it is a real node.
    pub fn node_unavailable(&self, node_id: NodeId) -> Option<UiMessage> {
        self.nodes.get(&node_id)?.unavailable()
    }

    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    pub fn node_definition(&self, node_id: NodeId) -> Option<NodeDefinition> {
        self.nodes.get(&node_id).map(|n| n.definition())
    }

    /// Node model for persistence/editor (position is managed by the server, defaulted here).
    pub fn node_model(&self, node_id: NodeId) -> Option<NodeModel> {
        self.nodes.get(&node_id).map(|n| NodeModel {
            id: node_id,
            type_name: n.definition().type_name,
            properties: n.properties().into_iter()
                .chain(self.input_values.get(&node_id).into_iter().flatten()
                    .map(|(pin, v)| (input_value_property(PinId(*pin)), v.clone())))
                .collect(),
            position: Position::default(),
        })
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> RemovedConnections {
        self.nodes.remove(&node_id);
        self.input_values.remove(&node_id);
        self.required_reported.remove(&node_id);
        self.data_cache.remove(&node_id);
        self.pure_cache.remove(&node_id);
        self.execution_queue.retain(|(n, _)| *n != node_id);

        self.host_state.cancel_node_tasks(node_id);

        let mut removed = RemovedConnections::default();
        self.exec_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.exec.push(*c); }
            keep
        });
        self.data_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.data.push(*c); }
            keep
        });
        removed
    }

    /// Sets a node property. If pins changed as a result, invalid connections are pruned.
    pub fn set_node_property(&mut self, node_id: NodeId, name: &str, value: DataValue) -> Result<RemovedConnections, EngineError> {
        if let Some(pin) = parse_input_value_property(name) {
            self.set_input_value(node_id, pin, Some(value))?;
            return Ok(RemovedConnections::default());
        }
        let node = self.nodes.get_mut(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        node.set_property(name, value);
        self.pure_cache.remove(&node_id);
        self.data_cache.remove(&node_id);

        // Input values whose pins disappeared or changed type are no longer valid
        let definition = node.definition();
        if let Some(values) = self.input_values.get_mut(&node_id) {
            values.retain(|pin, v| input_pin_type(&definition, PinId(*pin)).is_some_and(|t| input_value_fits(&self.types, &t, v)));
        }

        Ok(self.prune_connections(|c| c.src_node == node_id || c.dst_node == node_id))
    }

    /// Prunes connections touching selected nodes that have become invalid (pins changed).
    fn prune_connections(&mut self, touching: impl Fn(&ConnectionModel) -> bool) -> RemovedConnections {
        let invalid_exec: Vec<_> = self.exec_connections.iter()
            .filter(|c| touching(c) && self.validate_exec(c).is_err())
            .copied().collect();
        let invalid_data: Vec<_> = self.data_connections.iter()
            .filter(|c| touching(c) && self.validate_data_types(c).is_err())
            .copied().collect();
        self.exec_connections.retain(|c| !invalid_exec.contains(c));
        self.data_connections.retain(|c| !invalid_data.contains(c));
        RemovedConnections { exec: invalid_exec, data: invalid_data }
    }

    /// Value of an unconnected data input (`None` clears it). Effective while nothing is connected to the pin.
    pub fn set_input_value(&mut self, node_id: NodeId, pin: PinId, value: Option<DataValue>) -> Result<(), EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let values = self.input_values.entry(node_id).or_default();
        match value {
            Some(value) => {
                let pin_type = input_pin_type(&node.definition(), pin).ok_or(EngineError::InvalidPinConnection)?;
                if !input_value_fits(&self.types, &pin_type, &value) {
                    return Err(EngineError::TypeMismatch(value.get_type(), pin_type));
                }
                values.insert(pin.0, value);
            }
            None => {
                values.remove(&pin.0);
            }
        }
        self.pure_cache.clear();
        Ok(())
    }

    // --- Connections ---

    pub fn exec_connections(&self) -> &[ConnectionModel] {
        &self.exec_connections
    }

    pub fn data_connections(&self) -> &[ConnectionModel] {
        &self.data_connections
    }

    fn find_pin(&self, node_id: NodeId, pin_id: PinId, direction: PinDirection) -> Result<PinType, EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        node.definition().pins.into_iter()
            .find(|p| p.id == pin_id && p.direction == direction)
            .map(|p| p.pin_type)
            .ok_or(EngineError::InvalidPinConnection)
    }

    fn validate_exec(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Exec, PinType::Exec) => Ok(()),
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    fn validate_data_types(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Data(s_type), PinType::Data(d_type)) => {
                if self.types.compatible(&s_type, &d_type) {
                    Ok(())
                } else {
                    Err(EngineError::TypeMismatch(s_type, d_type))
                }
            }
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    pub fn connect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_exec(&c)?;
        if !self.exec_connections.contains(&c) {
            self.exec_connections.push(c);
        }
        Ok(())
    }

    pub fn disconnect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> bool {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        let before = self.exec_connections.len();
        self.exec_connections.retain(|x| *x != c);
        before != self.exec_connections.len()
    }

    /// Whether `start` depends (via data input pins) on `target`.
    fn has_data_path(&self, start: NodeId, target: NodeId) -> bool {
        let mut stack = vec![start];
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current == target { return true; }
            if !visited.insert(current) { continue; }
            for c in &self.data_connections {
                if c.dst_node == current { stack.push(c.src_node); }
            }
        }
        false
    }

    /// A data input pin has at most one source: a new connection replaces the previous one.
    pub fn connect_data(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_data_types(&c)?;
        if self.has_data_path(src_node, dst_node) {
            return Err(EngineError::CycleDetected(dst_node, src_node));
        }
        self.data_connections.retain(|x| x.dst_node != dst_node || x.dst_pin != dst_pin);
        self.data_connections.push(c);
        self.pure_cache.clear();
        Ok(())
    }

    pub fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId) -> bool {
        let before = self.data_connections.len();
        self.data_connections.retain(|c| c.dst_node != dst_node || c.dst_pin != dst_pin);
        self.pure_cache.clear();
        before != self.data_connections.len()
    }

    pub fn trigger_node(&mut self, node_id: NodeId, pin_id: PinId) {
        self.execution_queue.push_back((node_id, pin_id));
    }

    // --- Whole graph ---

    /// Exports in a deterministic order (friendly for git diffs).
    pub fn export_graph(&self) -> GraphModel {
        let mut ids = self.node_ids();
        ids.sort_by_key(|id| id.0);
        self.export_snippet(&ids)
    }

    pub fn export_snippet(&self, node_ids: &[NodeId]) -> GraphModel {
        let nodes = node_ids.iter().filter_map(|id| self.node_model(*id)).collect();
        let sort_key = |c: &ConnectionModel| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0);
        let mut exec_connections: Vec<_> = self.exec_connections.iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied().collect();
        exec_connections.sort_by_key(sort_key);
        let mut data_connections: Vec<_> = self.data_connections.iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied().collect();
        data_connections.sort_by_key(sort_key);
        GraphModel { nodes, exec_connections, data_connections }
    }

    /// Replaces the current graph with the saved one. Returns warnings for elements that could not be restored.
    pub fn load_graph(&mut self, graph: &GraphModel) -> Vec<String> {
        for id in self.node_ids() {
            self.remove_node(id);
        }
        self.execution_queue.clear();
        self.variables.clear();
        self.insert_graph(graph, |id| id)
    }

    /// Inserts a graph snippet with fresh IDs (copy/paste).
    /// Returns `(snippet_id, new_id)` pairs for created nodes along with warnings.
    /// Connections to nodes outside the snippet are ignored.
    pub fn import_snippet(&mut self, snippet: &GraphModel) -> (Vec<(NodeId, NodeId)>, Vec<String>) {
        let id_map: HashMap<NodeId, NodeId> = snippet.nodes.iter().map(|n| (n.id, NodeId::generate())).collect();
        let inside = |c: &&ConnectionModel| id_map.contains_key(&c.src_node) && id_map.contains_key(&c.dst_node);
        let snippet = GraphModel {
            nodes: snippet.nodes.clone(),
            exec_connections: snippet.exec_connections.iter().filter(inside).copied().collect(),
            data_connections: snippet.data_connections.iter().filter(inside).copied().collect(),
        };
        let warnings = self.insert_graph(&snippet, |id| id_map[&id]);
        let created = snippet.nodes.iter().map(|n| (n.id, id_map[&n.id])).filter(|(_, new)| self.has_node(*new)).collect();
        (created, warnings)
    }

    fn insert_graph(&mut self, graph: &GraphModel, map_id: impl Fn(NodeId) -> NodeId) -> Vec<String> {
        let mut warnings = Vec::new();
        let mut unknown = Vec::new();
        for n in &graph.nodes {
            match self.create_node(map_id(n.id), &n.type_name, &n.properties) {
                Ok(()) => {}
                Err(EngineError::UnknownNodeType(_)) => unknown.push(n),
                Err(e) => warnings.push(format!("Node {:?}: {}", n.id, e)),
            }
        }
        // An unknown type (plugin disabled or not loaded) does not lose the node: a stub preserves
        // properties and connections so the graph saves back without data loss
        for n in unknown {
            let id = map_id(n.id);
            let pins = self.infer_stub_pins(id, graph, &map_id);
            tracing::warn!(node = ?n.id, "Unknown node type {}: created a stub", n.type_name);
            self.add_node(Box::new(StubNode::new(id, StubNode::bare_definition(&n.type_name, pins), n.properties.clone(), None)));
        }
        for c in &graph.exec_connections {
            if let Err(e) = self.connect_exec(map_id(c.src_node), c.src_pin, map_id(c.dst_node), c.dst_pin) {
                warnings.push(format!("Exec connection {:?}: {}", c, e));
            }
        }
        for c in &graph.data_connections {
            if let Err(e) = self.connect_data(map_id(c.src_node), c.src_pin, map_id(c.dst_node), c.dst_pin) {
                warnings.push(format!("Data connection {:?}: {}", c, e));
            }
        }
        warnings
    }

    // --- Execution ---

    fn pull_data(&mut self, node_id: NodeId, pin_id: PinId, stack: &mut Vec<NodeId>) -> Option<DataValue> {
        if stack.contains(&node_id) { return None; }

        // Check whether this is an input pin connected to another node's output.
        // If so, recursively pull data from the source.
        let incoming = self.data_connections.iter()
            .find(|c| c.dst_node == node_id && c.dst_pin == pin_id)
            .copied();
        if let Some(c) = incoming {
            stack.push(node_id);
            let val = self.pull_data(c.src_node, c.src_pin, stack);
            stack.pop();
            return val;
        }

        if let Some(cached_node_vals) = self.pure_cache.get(&node_id) {
            if let Some(val) = cached_node_vals.get(&pin_id) {
                return Some(val.clone());
            }
        }

        let is_pure = self.nodes.get(&node_id).map(|n| n.is_pure()).unwrap_or(false);
        if is_pure {
            stack.push(node_id);
            let mut node = self.nodes.remove(&node_id)?;
            let inputs = self.pull_inputs(node_id, stack);
            if self.skip_for_required(node_id, &node.definition(), &inputs) {
                self.nodes.insert(node_id, node);
                stack.pop();
                return None;
            }
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let outputs = node.execute(None, &mut ctx);
            self.nodes.insert(node_id, node);
            stack.pop();

            let mut node_outputs = HashMap::new();
            let mut target_val = None;
            for out in outputs {
                if let NodeOutput::Data(p, v) = out {
                    if p == pin_id {
                        target_val = Some(v.clone());
                    }
                    node_outputs.insert(p, v);
                }
            }
            self.pure_cache.entry(node_id).or_default().extend(node_outputs);
            self.record_trace(node_id, traced, Vec::new());
            target_val
        } else {
            self.data_cache.get(&node_id).and_then(|m| m.get(&pin_id)).cloned()
        }
    }

    fn pull_inputs(&mut self, node_id: NodeId, stack: &mut Vec<NodeId>) -> HashMap<PinId, Option<DataValue>> {
        let connections: Vec<_> = self.data_connections.iter().filter(|c| c.dst_node == node_id).copied().collect();
        let mut inputs: HashMap<PinId, Option<DataValue>> = connections.into_iter()
            .map(|c| (c.dst_pin, self.pull_data(c.src_node, c.src_pin, stack)))
            .collect();
        for (pin, value) in self.input_values.get(&node_id).into_iter().flatten() {
            inputs.entry(PinId(*pin)).or_insert_with(|| Some(value.clone()));
        }
        inputs
    }

    /// A node with an unset required input is not executed. An input is considered set if a source is
    /// connected to it or an inline value is set on the node. A connected source with no value yet skips
    /// the node silently; a missing input logs an error (once, until the state changes).
    fn skip_for_required(&mut self, node_id: NodeId, def: &NodeDefinition, inputs: &HashMap<PinId, Option<DataValue>>) -> bool {
        let mut pending = false;
        for pin in def.pins.iter().filter(|p| p.required && p.direction == PinDirection::Input && p.pin_type != PinType::Exec) {
            match inputs.get(&pin.id) {
                Some(Some(_)) => {}
                Some(None) => pending = true,
                None => {
                    if self.required_reported.insert(node_id) {
                        self.host_state.log(node_id, format!("[ERROR] Required input \"{}\" is not set", pin.name));
                    }
                    return true;
                }
            }
        }
        if !pending {
            self.required_reported.remove(&node_id);
        }
        pending
    }

    pub fn tick(&mut self) {
        self.pure_cache.clear();
        self.executed.clear();
        self.host_state.current_tick += 1;
        let mut next_tick_queue = VecDeque::new();

        // Completed tasks (delays, HTTP) are delivered to their owning nodes.
        for (node_id, task, result) in self.host_state.take_finished() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.on_task(task, result, &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }

        let mut fired = Vec::new();
        for (id, node) in self.nodes.iter_mut() {
            if let Some(pin) = node.poll(&mut self.host_state) {
                fired.push((*id, pin));
            }
        }
        for (id, pin) in fired {
            self.trigger_node(id, pin);
        }

        let mut current_tick_queue = std::mem::take(&mut self.execution_queue);
        while let Some((node_id, trigger_pin)) = current_tick_queue.pop_front() {
            if !self.nodes.contains_key(&node_id) { continue; }
            let mut stack = Vec::new();
            let inputs = self.pull_inputs(node_id, &mut stack);
            if let Some(def) = self.node_definition(node_id) && self.skip_for_required(node_id, &def, &inputs) {
                continue;
            }
            let traced = trace_inputs(&inputs);
            let mut ctx = NodeContext { node_id, inputs, variables: &self.variables, host_state: &mut self.host_state };
            let Some(node) = self.nodes.get_mut(&node_id) else { continue };
            let outputs = node.execute(Some(trigger_pin), &mut ctx);
            self.apply_outputs(node_id, traced, outputs, &mut next_tick_queue);
        }
        self.execution_queue.extend(next_tick_queue);
    }

    /// Exec outputs are queued for the next tick; data and variables are applied immediately.
    fn apply_outputs(
        &mut self,
        node_id: NodeId,
        inputs: Vec<(PinId, Option<DataValue>)>,
        outputs: Vec<NodeOutput>,
        next_tick_queue: &mut VecDeque<(NodeId, PinId)>,
    ) {
        let mut state_changed = false;
        let mut fired = Vec::new();
        for output in outputs {
            match output {
                NodeOutput::Execution(out_exec_pin) => {
                    fired.push(out_exec_pin);
                    for c in &self.exec_connections {
                        if c.src_node == node_id && c.src_pin == out_exec_pin {
                            next_tick_queue.push_back((c.dst_node, c.dst_pin));
                        }
                    }
                }
                NodeOutput::Data(pin_id, value) => {
                    self.data_cache.entry(node_id).or_default().insert(pin_id, value);
                    state_changed = true;
                }
                NodeOutput::Variable(name, value) => {
                    self.variables.insert(name, value);
                    state_changed = true;
                }
            }
        }
        // Pure nodes evaluated earlier in this tick might depend on the updated data.
        if state_changed {
            self.pure_cache.clear();
        }
        self.record_trace(node_id, inputs, fired);
    }

    /// If a node executed multiple times in a tick, the last execution's data is kept while fired exec pins accumulate.
    fn record_trace(&mut self, node_id: NodeId, inputs: Vec<(PinId, Option<DataValue>)>, fired: Vec<PinId>) {
        let mut outputs: Vec<(PinId, DataValue)> = self.data_cache.get(&node_id).into_iter().flatten()
            .chain(self.pure_cache.get(&node_id).into_iter().flatten())
            .map(|(p, v)| (*p, v.clone()))
            .collect();
        outputs.sort_by_key(|(p, _)| p.0);
        outputs.dedup_by_key(|(p, _)| *p);
        let tick = self.host_state.current_tick;
        match self.executed.iter_mut().find(|t| t.node_id == node_id) {
            Some(trace) => {
                trace.inputs = inputs;
                trace.outputs = outputs;
                for pin in fired {
                    if !trace.fired.contains(&pin) {
                        trace.fired.push(pin);
                    }
                }
            }
            None => self.executed.push(NodeTrace { node_id, tick, inputs, outputs, fired }),
        }
    }
}

/// Data type of an input pin (`None` if no such pin exists or it is an exec pin).
fn input_pin_type(definition: &NodeDefinition, pin: PinId) -> Option<DataType> {
    definition.pins.iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Input)
        .and_then(|p| match &p.pin_type {
            PinType::Data(t) => Some(t.clone()),
            PinType::Exec => None,
        })
}

fn trace_inputs(inputs: &HashMap<PinId, Option<DataValue>>) -> Vec<(PinId, Option<DataValue>)> {
    let mut traced: Vec<_> = inputs.iter().map(|(p, v)| (*p, v.clone())).collect();
    traced.sort_by_key(|(p, _)| p.0);
    traced
}

// --- BUILT-IN NODE IMPLEMENTATIONS ---

pub struct ConstNode { pub id: NodeId, pub value: DataValue }
impl Node for ConstNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "const".to_string(),
            title: "Constant".to_string(),
            category: "Data".to_string(),
            description: "Returns a constant value".to_string(),
            help: "Emits the value set directly on the node (\"Value\" row) or in the right panel on output Val. The value type (number, string, boolean, etc.) is chosen on the node, and the pin type follows it.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(self.value.get_type()),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, _:&mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), self.value.clone())]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("value".to_string(), self.value.clone())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value" {
            self.value = value;
        }
    }
}

pub struct PureMathNode { pub id: NodeId, pub op: MathOp }
impl Node for PureMathNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let name = match self.op {
            MathOp::Add => "math/add", MathOp::Sub => "math/sub", MathOp::Mul => "math/mul", MathOp::Div => "math/div",
        };
        let title = match self.op {
            MathOp::Add => "Math Add", MathOp::Sub => "Math Subtract", MathOp::Mul => "Math Multiply", MathOp::Div => "Math Divide",
        };
        NodeDefinition {
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Math".to_string(),
            description: "Arithmetic operation on two numbers".to_string(),
            help: match self.op {
                MathOp::Add => "Res = A + B.",
                MathOp::Sub => "Res = A − B.",
                MathOp::Mul => "Res = A × B.",
                MathOp::Div => "Res = A ÷ B. Division by 0 yields infinity.",
            }.to_string() + "\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.",
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0));
        let b = ctx.get_number(PinId(1), NumberValue::Int(0));
        let res = match self.op {
            MathOp::Add => a + b, MathOp::Sub => a - b, MathOp::Mul => a * b, MathOp::Div => a / b,
        };
        vec![NodeOutput::Data(PinId(2), DataValue::Number(res))]
    }
}

pub struct MathExpressionNode {
    pub id: NodeId,
    pub expression: String,
    pub variable_mapping: HashMap<String, PinId>,
}
impl Node for MathExpressionNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: "Res".to_string(),
            pin_type: PinType::Data(DataType::Number),
            direction: PinDirection::Output, required: false }];
        let mut vars: Vec<_> = self.variable_mapping.iter().collect();
        vars.sort_by_key(|(_, pin)| pin.0);
        for (name, pin_id) in vars {
            pins.push(PinDefinition {
                id: *pin_id,
                name: name.clone(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Input, required: false });
        }
        NodeDefinition {
            type_name: "math/expression".to_string(),
            title: "Expression".to_string(),
            category: "Math".to_string(),
            description: "Evaluates an arbitrary mathematical formula".to_string(),
            help: "Evaluates the formula set directly on the node (\"Formula\" row), such as `(a + b) / 2` or `x * x - 1`.\n\nEach variable in the formula automatically becomes an input pin (in alphabetical order). An unconnected variable uses the value set next to the pin, or defaults to 0. If the formula is invalid, the result is 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let mut context = HashMapContext::new();
        for (name, pin) in &self.variable_mapping {
            let val = ctx.get_number(*pin, NumberValue::Int(0));
            let _ = context.set_value(name.clone().into(), Value::from(val.to_f64()));
        }
        let result = match eval_with_context(&self.expression, &context) {
            Ok(val) => {
                if let Ok(f) = val.as_float() { NumberValue::Float(f).normalize() }
                else if let Ok(i) = val.as_int() { NumberValue::Int(i).normalize() }
                else { NumberValue::Int(0) }
            }
            Err(_) => NumberValue::Int(0),
        };
        vec![NodeOutput::Data(PinId(0), DataValue::Number(result))]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("expression".to_string(), DataValue::String(self.expression.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "expression" {
            if let DataValue::String(s) = value {
                self.variable_mapping = expression_variables(&s);
                self.expression = s;
            }
        }
    }
}

/// Formula variables become input pins (1, 2, ...) in alphabetical order.
pub fn expression_variables(expression: &str) -> HashMap<String, PinId> {
    let mut names: Vec<String> = match build_operator_tree(expression) {
        Ok(tree) => tree.iter_read_variable_identifiers().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names.dedup();
    names.into_iter().enumerate().map(|(i, n)| (n, PinId(i as u32 + 1))).collect()
}

pub struct LogicalNode { pub id: NodeId, pub op: LogOp }
impl Node for LogicalNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let name = match self.op {
            LogOp::And => "logical/and", LogOp::Or => "logical/or", LogOp::Not => "logical/not",
        };
        let title = match self.op {
            LogOp::And => "Logical And", LogOp::Or => "Logical Or", LogOp::Not => "Logical Not",
        };
        let pins = match self.op {
            LogOp::And | LogOp::Or => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
            LogOp::Not => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        };
        NodeDefinition {
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Logic".to_string(),
            description: "Logical operation".to_string(),
            help: match self.op {
                LogOp::And => "Res = true only if both A and B are true.",
                LogOp::Or => "Res = true if at least one of A or B is true.",
                LogOp::Not => "Res is the logical negation of A.",
            }.to_string() + "\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to false.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.",
            pins,
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_bool(PinId(0), false);
        let b = ctx.get_bool(PinId(1), false);
        let res = match self.op { LogOp::And => a && b, LogOp::Or => a || b, LogOp::Not => !a };
        vec![NodeOutput::Data(PinId(2), DataValue::Bool(res))]
    }
}

pub struct VariableGetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "variable/get".to_string(),
            title: format!("Get {}", self.var_name),
            category: "Data".to_string(),
            description: "Returns the value of a variable by name".to_string(),
            help: "Reads the global variable named in the \"Variable\" property and outputs it on Val. Variables are shared across the entire graph and written by the Set node. Until the variable is written, outputs 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_var(&self.var_name).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Data(PinId(0), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}

pub struct VariableSetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableSetNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "variable/set".to_string(),
            title: format!("Set {}", self.var_name),
            category: "Data".to_string(),
            description: "Writes a value to a variable".to_string(),
            help: "On a signal at In, writes the value from Val to the global variable (named in the \"Variable\" property) and passes execution to Out.\n\nThe variable is visible to all Get nodes with the same name across the entire graph.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(0), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1)).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Execution(PinId(0)), NodeOutput::Variable(self.var_name.clone(), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}

/// Decider combinator: on an In signal, evaluates cases in order and routes execution to the output
/// of the first matching case (or all matching cases), or to Else if none match.
/// Inputs are an explicit list (`inputs`: pin, name, type); in older graphs without it, names are inferred from conditions.
pub struct MatchNode {
    pub id: NodeId,
    pub cases: Vec<MatchCase>,
    /// Fire all matching cases rather than only the first one.
    pub all_matches: bool,
    inputs: Vec<MatchInput>,
    /// Inputs are defined explicitly (otherwise inferred from conditions, as in legacy graphs).
    explicit_inputs: bool,
}

impl MatchNode {
    pub const ELSE_PIN: PinId = PinId(0);

    pub fn new(id: NodeId) -> Self {
        // Default input `a` is inferred from the condition. While the node is being restored, cases without `inputs`
        // (legacy format) infer inputs; after `restored`, inputs are explicit — both for new and loaded nodes.
        let mut node = Self { id, cases: Vec::new(), all_matches: false, inputs: Vec::new(), explicit_inputs: false };
        node.set_cases(vec![MatchCase::parse("1: a > 0").expect("the default case is valid")]);
        node
    }

    /// Case pin numbers are unique and do not collide with Else.
    fn set_cases(&mut self, mut cases: Vec<MatchCase>) {
        let mut seen = std::collections::HashSet::new();
        cases.retain(|c| c.pin != Self::ELSE_PIN.0 && seen.insert(c.pin));
        self.cases = cases;
        if !self.explicit_inputs {
            self.inputs = match_inputs(&self.cases).into_iter().enumerate()
                .map(|(i, name)| MatchInput { pin: i as u32 + 1, name, value_type: DataType::Any })
                .collect();
        }
    }

    /// Input pins and names are unique; pin 0 is reserved for the In exec input.
    fn set_inputs(&mut self, mut inputs: Vec<MatchInput>) {
        let mut pins = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        inputs.retain(|i| i.pin != 0 && pins.insert(i.pin) && names.insert(i.name.clone()));
        self.inputs = inputs;
        self.explicit_inputs = true;
    }
}

fn string_list(elements: &[DataValue]) -> Option<Vec<&str>> {
    elements.iter().map(|e| match e {
        DataValue::String(s) => Some(s.as_str()),
        _ => None,
    }).collect()
}

impl Node for MatchNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false }];
        pins.extend(self.inputs.iter().map(|i| PinDefinition {
            id: PinId(i.pin),
            name: i.name.clone(),
            pin_type: PinType::Data(i.value_type.clone()),
            direction: PinDirection::Input, required: false }));
        pins.extend(self.cases.iter().map(|c| PinDefinition {
            id: PinId(c.pin),
            name: c.display(),
            pin_type: PinType::Exec,
            direction: PinDirection::Output, required: false }));
        pins.push(PinDefinition { id: Self::ELSE_PIN, name: "Else".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false });
        NodeDefinition {
            type_name: "match".to_string(),
            title: "Match".to_string(),
            category: "Control Flow".to_string(),
            description: "Selects an output by conditions, like a decider combinator".to_string(),
            help: "On a signal at In, checks cases from top to bottom and passes execution to the output of the first matching case. If none match — to Else. With \"All matches\" enabled, all matching cases fire.\n\nInputs: \"+ input\" adds an input pin. The input name can be renamed (conditions update automatically) and its type selected; \"auto\" accepts anything, picking operators based on the connected source type. An unconnected input can be given a value directly on the node.\n\nA condition is \"input  operator  operand\". Operators depend on the input type:\n• numbers, time, duration — =, ≠, <, ≤, >, ≥ (for time — earlier / later);\n• strings — equals, contains, starts with, ends with, is empty, length;\n• booleans — = true / = false;\n• lists — contains, is empty, length;\n• objects — has field;\n• any — has value / no value.\nThe right side is a value or another input (comparing two variables). Values are coerced to the input type: \"07:30\" — time, \"1h 30m\" or 90 — duration, \"#ff0000\" — color.\n\nConditions are joined with \"and\" / \"or\"; \"and\" binds tighter: \"a > 1 and b > 1 or c\" = \"(a > 1 and b > 1) or c\". A case with no conditions always matches. An input without a value does not satisfy a condition (except \"no value\").".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value_of = |name: &str| {
            self.inputs.iter().find(|i| i.name == name).and_then(|i| ctx.get_value(PinId(i.pin)))
        };
        let mut outputs = Vec::new();
        for case in &self.cases {
            if case.matches(value_of) {
                outputs.push(NodeOutput::Execution(PinId(case.pin)));
                if !self.all_matches {
                    break;
                }
            }
        }
        if outputs.is_empty() {
            outputs.push(NodeOutput::Execution(Self::ELSE_PIN));
        }
        outputs
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        let list = |items: Vec<String>| DataValue::List {
            element_type: DataType::String,
            elements: items.into_iter().map(DataValue::String).collect(),
        };
        // Inputs come before cases: when reconstructing the node, cases need not infer inputs on their own
        vec![
            ("inputs".to_string(), list(self.inputs.iter().map(MatchInput::to_text).collect())),
            ("cases".to_string(), list(self.cases.iter().map(MatchCase::to_text).collect())),
            ("all_matches".to_string(), DataValue::Bool(self.all_matches)),
        ]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        // An invalid list is not applied at all (the editor validates it beforehand)
        match (name, value) {
            ("cases", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "a case must be a string".to_string())
                    .and_then(|l| l.into_iter().map(MatchCase::parse).collect());
                match parsed {
                    Ok(cases) => self.set_cases(cases),
                    Err(e) => tracing::warn!("Match: invalid cases: {e}"),
                }
            }
            ("inputs", DataValue::List { elements, .. }) => {
                let parsed: Result<Vec<_>, _> = string_list(&elements)
                    .ok_or_else(|| "an input must be a string".to_string())
                    .and_then(|l| l.into_iter().map(MatchInput::parse).collect());
                match parsed {
                    Ok(inputs) => self.set_inputs(inputs),
                    Err(e) => tracing::warn!("Match: invalid inputs: {e}"),
                }
            }
            ("all_matches", DataValue::Bool(b)) => self.all_matches = b,
            _ => {}
        }
    }
    fn restored(&mut self) {
        self.explicit_inputs = true;
    }
}

pub struct ClockNode { pub id: NodeId, pub interval_ticks: u64 }
impl Node for ClockNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "clock".to_string(),
            title: "Clock".to_string(),
            category: "Events".to_string(),
            description: "Emits an execution pulse every interval_ticks ticks".to_string(),
            help: "Triggers execution on its own: every N ticks (\"Interval\" property) it emits a signal on Out.\n\nThe server tick is 100 ms, so 10 ticks ≈ 1 second. This is the primary event source for chains that run continuously.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Out".to_string(),
                pin_type: PinType::Exec,
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
        (host.current_tick % self.interval_ticks.max(1) == 0).then_some(PinId(0))
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "interval_ticks" {
            if let DataValue::Number(n) = value {
                self.interval_ticks = n.to_f64().max(1.0) as u64;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("interval_ticks".to_string(), DataValue::Number(NumberValue::Int(self.interval_ticks as i64)))]
    }
}

pub struct ConsoleNode { pub id: NodeId }
impl Node for ConsoleNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "console".to_string(),
            title: "Console Log".to_string(),
            category: "Debug".to_string(),
            description: "Prints a value to the console when triggered".to_string(),
            help: "On a signal at In, prints the value of Val to the editor console and server log. Without Val connected, prints \"None\".\n\nHas no output — this is a terminal node in a chain. To log a value in the middle of a chain, use Debug Log.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let message = val.as_ref().map(format_data_value).unwrap_or_else(|| "None".to_string());
        info!(%message, "Console Output");
        ctx.host_state.logs.push((self.id, message));
        vec![]
    }
}

pub struct ClickButtonNode {
    pub id: NodeId,
    pub label: String,
    pub clicked: bool,
}

impl Node for ClickButtonNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/click_button".to_string(),
            title: "Click Button".to_string(),
            category: "Events".to_string(),
            description: "Emits an exec signal on the next tick after the user clicks the button".to_string(),
            help: "Emits a signal on Out when you click the Run button on the node.\n\nHandy for manually triggering a chain while debugging.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Out".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        self.clicked = false;
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "label" {
            if let DataValue::String(s) = value {
                self.label = s;
            }
        } else if name == "clicked" {
            if let DataValue::Bool(b) = value {
                self.clicked = b;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("label".to_string(), DataValue::String(self.label.clone()))]
    }
}

pub fn format_data_value(val: &DataValue) -> String {
    match val {
        DataValue::Bool(b) => b.to_string(),
        DataValue::Number(num) => match num {
            NumberValue::Int(i) => i.to_string(),
            NumberValue::Float(f) => f.to_string(),
        },
        DataValue::String(s) => s.clone(),
        DataValue::Time(t) => t.to_string(),
        DataValue::DateTime(dt) => dt.to_string(),
        DataValue::Duration(d) => format!("{:?}", d),
        DataValue::Color(c) => format!("Color(r:{}, g:{}, b:{}, a:{})", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => {
            let items: Vec<String> = elements.iter().map(format_data_value).collect();
            format!("[{}]", items.join(", "))
        }
        DataValue::Object { fields, .. } => {
            let mut items: Vec<String> = fields.iter()
                .map(|(k, v)| format!("{}: {}", k, format_data_value(&v.value)))
                .collect();
            items.sort();
            format!("{{{}}}", items.join(", "))
        }
    }
}

pub struct DebugNode {
    pub id: NodeId,
    pub prefix: String,
}

impl Node for DebugNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/debug".to_string(),
            title: "Debug Log".to_string(),
            category: "Debug".to_string(),
            description: "Accepts any data and logs it with a text prefix".to_string(),
            help: "On a signal at In, logs the \"Prefix\" together with the value of Val to the console and passes execution to Out.\n\nCan be inserted in the middle of a chain to inspect the value passing through it.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let formatted = match val {
            Some(v) => format_data_value(&v),
            None => "None".to_string(),
        };
        let message = format!("{}{}", self.prefix, formatted);
        info!("{}", message);
        ctx.host_state.logs.push((self.id, message));
        vec![NodeOutput::Execution(PinId(2))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "prefix" {
            if let DataValue::String(s) = value {
                self.prefix = s;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("prefix".to_string(), DataValue::String(self.prefix.clone()))]
    }
}

// Helper to parse fields_list
fn parse_fields_list(s: &str) -> Vec<(String, DataType)> {
    let mut fields = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((name, ty_str)) = part.split_once(':') {
            let name = name.trim().to_string();
            let ty = parse_data_type(ty_str).unwrap_or(DataType::String);
            fields.push((name, ty));
        } else {
            fields.push((part.to_string(), DataType::String));
        }
    }
    fields
}

pub struct CompareNode { pub id: NodeId }
impl Node for CompareNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "compare".to_string(),
            title: "Compare".to_string(),
            category: "Logic".to_string(),
            description: "Compares two numbers".to_string(),
            help: "Compares numbers A and B:\n• Eq — A equals B (within 1e-9);\n• Gt — A is greater than B;\n• Lt — A is less than B.\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0.\n\nFor branching execution on a condition, the Match node is more convenient.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Eq".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Gt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(4), name: "Lt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0)).to_f64();
        let b = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64();
        vec![
            NodeOutput::Data(PinId(2), DataValue::Bool((a - b).abs() < 1e-9)),
            NodeOutput::Data(PinId(3), DataValue::Bool(a > b)),
            NodeOutput::Data(PinId(4), DataValue::Bool(a < b)),
        ]
    }
}

pub struct ForLoopNode {
    pub id: NodeId,
    pub active: bool,
    pub current_index: i64,
    pub count: i64,
}
impl Node for ForLoopNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "for_loop".to_string(),
            title: "For Loop".to_string(),
            category: "Control Flow".to_string(),
            description: "Asynchronous loop from 0 to Count-1 with step 1".to_string(),
            help: "Step-by-step loop:\n1. A signal at In starts the loop with Index = 0 and fires LoopBody.\n2. When the loop body finishes, send a signal to Next — Index increments by 1 and LoopBody fires again.\n3. After Count iterations or a signal at Break, Completed fires.\n\nIf Count ≤ 0, Completed fires immediately. Each step takes at least one tick, so the loop never blocks the engine.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Count".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Break".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "Next".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "LoopBody".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(5), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(6), name: "Completed".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let count_val = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64() as i64;
            if count_val <= 0 {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                self.active = true;
                self.current_index = 0;
                self.count = count_val;
                vec![
                    NodeOutput::Execution(PinId(4)),
                    NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(0))),
                ]
            }
        } else if trigger_pin == Some(PinId(2)) {
            if self.active {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                vec![]
            }
        } else if trigger_pin == Some(PinId(3)) {
            if self.active {
                self.current_index += 1;
                if self.current_index >= self.count {
                    self.active = false;
                    vec![NodeOutput::Execution(PinId(6))]
                } else {
                    vec![
                        NodeOutput::Execution(PinId(4)),
                        NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(self.current_index))),
                    ]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}

pub struct SleepNode { pub id: NodeId }
impl Node for SleepNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "sleep".to_string(),
            title: "Sleep".to_string(),
            category: "Time".to_string(),
            description: "Delays execution by the specified number of milliseconds or ticks".to_string(),
            help: "A signal at In starts a delay, and when it elapses, emits a signal on Out.\n\n• Delay — duration in milliseconds (rounded up to whole ticks).\n• UseTicks = true — Delay is counted in ticks.\n• TickDuration — tick duration in ms for calculation (defaults to the server tick).\n\nEach signal at In starts its own independent delay, like Queue Delay.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Delay".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "UseTicks".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "TickDuration".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let delay = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64().max(0.0);
            let ticks = if ctx.get_bool(PinId(2), false) {
                delay.ceil() as u64
            } else {
                // Milliseconds; TickDuration (if set) overrides the engine's tick duration.
                match ctx.get_number(PinId(3), NumberValue::Int(0)).to_f64() {
                    tick_ms if tick_ms > 0.0 => (delay / tick_ms).ceil() as u64,
                    _ => ctx.host_state.ticks_for(std::time::Duration::from_millis(delay as u64)),
                }
            };
            ctx.host_state.start_delay(self.id, ticks);
        }
        vec![]
    }
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(4))]
    }
}

pub struct ListCreateNode {
    pub id: NodeId,
    pub element_type: DataType,
}
impl Node for ListCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/create".to_string(),
            title: "Create List".to_string(),
            category: "Lists".to_string(),
            description: "Creates an empty list".to_string(),
            help: "Outputs an empty list. The element type is selected on the node.\n\nPopulate the list using the List Add node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "List".to_string(),
                    pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))),
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(
            PinId(0),
            DataValue::List {
                element_type: self.element_type.clone(),
                elements: Vec::new(),
            }
        )]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListAddNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListAddNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/add".to_string(),
            title: "List Add".to_string(),
            category: "Lists".to_string(),
            description: "Appends an item to a list".to_string(),
            help: "Outputs OutList — a copy of List with Item appended at the end. The input list is not modified.\n\nThe element type is selected on the node and must match the list type.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let item_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::List { element_type, mut elements }), Some(item)) = (list_val, item_val) {
            elements.push(item);
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListRemoveNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListRemoveNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/remove".to_string(),
            title: "List Remove".to_string(),
            category: "Lists".to_string(),
            description: "Removes an item from a list by index".to_string(),
            help: "Outputs OutList — a copy of the list without the element at Index (0-based). If the index is out of bounds, the list is returned unchanged.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { element_type, mut elements }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                elements.remove(index_val as usize);
            }
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListGetNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/get".to_string(),
            title: "List Get".to_string(),
            category: "Lists".to_string(),
            description: "Gets an item from a list by index".to_string(),
            help: "Outputs Item — the list element at Index (0-based). If the index is out of bounds, the output has no value.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { elements, .. }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                vec![NodeOutput::Data(PinId(2), elements[index_val as usize].clone())]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListLengthNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListLengthNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "list/length".to_string(),
            title: "List Length".to_string(),
            category: "Lists".to_string(),
            description: "Returns the length of a list".to_string(),
            help: "Outputs Len — the number of elements in the list. If no list is connected, outputs 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Len".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        if let Some(DataValue::List { elements, .. }) = list_val {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(elements.len() as i64)))]
        } else {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(0)))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "element_type" {
            if let DataValue::String(s) = value {
                self.element_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ObjectCreateNode { pub id: NodeId }
impl Node for ObjectCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/create".to_string(),
            title: "Create Object".to_string(),
            category: "Objects".to_string(),
            description: "Creates an empty object".to_string(),
            help: "Outputs an empty object — a collection of named fields.\n\nAdd fields using the Set Object Field node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), DataValue::Object { fields: std::collections::BTreeMap::new(), type_name: None })]
    }
}

pub struct ObjectSetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectSetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/set_field".to_string(),
            title: "Set Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Sets the value of an object field".to_string(),
            help: "Outputs OutObj — a copy of object Obj where field Name has value Val. If the field did not exist, it is added; otherwise it is replaced.\n\nThe type of Val is selected on the node. Until all inputs are provided, no output is produced.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Name".to_string(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "OutObj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        let val = ctx.get_value(PinId(2));
        if let (Some(DataValue::Object { mut fields, .. }), Some(DataValue::String(name)), Some(v)) = (obj_val, name_val, val) {
            fields.insert(name.clone(), ObjectField {
                name,
                description: String::new(),
                value: v,
            });
            vec![NodeOutput::Data(PinId(3), DataValue::Object { fields, type_name: None })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "field_type" {
            if let DataValue::String(s) = value {
                self.field_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectGetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectGetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "object/get_field".to_string(),
            title: "Get Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Gets the value of an object field".to_string(),
            help: "Looks up field Name in object Obj:\n• Val — the field value;\n• Found — whether the field exists.\n\nThe type of Val is selected on the node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Name".to_string(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Found".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::Object { fields, .. }), Some(DataValue::String(name))) = (obj_val, name_val) {
            if let Some(field) = fields.get(&name) {
                vec![
                    NodeOutput::Data(PinId(2), field.value.clone()),
                    NodeOutput::Data(PinId(3), DataValue::Bool(true)),
                ]
            } else {
                vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
            }
        } else {
            vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "field_type" {
            if let DataValue::String(s) = value {
                self.field_type = parse_data_type(&s).unwrap_or(DataType::Number);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectUnpackNode {
    pub id: NodeId,
    pub fields_list: String,
    pub parsed_fields: Vec<(String, DataType)>,
}
impl Node for ObjectUnpackNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![
            PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
        ];
        for (i, (name, ty)) in self.parsed_fields.iter().enumerate() {
            pins.push(PinDefinition {
                id: PinId((i + 1) as u32),
                name: name.clone(),
                pin_type: PinType::Data(ty.clone()),
                direction: PinDirection::Output, required: false });
        }
        NodeDefinition {
            type_name: "object/unpack".to_string(),
            title: "Unpack Object".to_string(),
            category: "Objects".to_string(),
            description: "Unpacks fields of an object".to_string(),
            help: "Outputs several object fields at once. Fields are listed on the node or in the right panel separated by commas as \"name:type\", for example `temp:number, city:string` (without a type — string).\n\nEach field becomes a separate output pin. A field missing from the object has no value.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let mut outputs = Vec::new();
        if let Some(DataValue::Object { fields, .. }) = obj_val {
            for (i, (name, _)) in self.parsed_fields.iter().enumerate() {
                if let Some(field) = fields.get(name) {
                    outputs.push(NodeOutput::Data(PinId((i + 1) as u32), field.value.clone()));
                }
            }
        }
        outputs
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "fields_list" {
            if let DataValue::String(s) = value {
                self.fields_list = s;
                self.parsed_fields = parse_fields_list(&self.fields_list);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("fields_list".to_string(), DataValue::String(self.fields_list.clone()))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sink node with exec input PinId(0) for tracing execution flow.
    fn add_sink(engine: &mut TickEngine) -> NodeId {
        let id = NodeId::generate();
        engine.create_node(id, "test/debug", &[]).unwrap();
        id
    }

    fn cases(list: &[&str]) -> (String, DataValue) {
        ("cases".to_string(), DataValue::List {
            element_type: DataType::String,
            elements: list.iter().map(|c| DataValue::String(c.to_string())).collect(),
        })
    }

    fn num(v: i64) -> DataValue {
        DataValue::Number(NumberValue::Int(v))
    }

    /// Triggers the node and returns the exec pins that fired.
    fn fire(engine: &mut TickEngine, id: NodeId) -> Vec<PinId> {
        engine.trigger_node(id, PinId(0));
        engine.tick();
        engine.executed.iter().find(|t| t.node_id == id).map(|t| t.fired.clone()).unwrap_or_default()
    }

    #[test]
    fn test_match_cases_and_else() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[cases(&["1: t > 25", "2: t > 20 and mode = \"auto\"", "3: t < 0 or t > 100"])]).unwrap();
        let def = engine.node_definition(id).unwrap();
        let inputs: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Input).map(|p| p.name.as_str()).collect();
        assert_eq!(inputs, ["In", "mode", "t"]);
        let outputs: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Output).map(|p| p.name.as_str()).collect();
        assert_eq!(outputs, ["t > 25", "t > 20 and mode = \"auto\"", "t < 0 or t > 100", "Else"]);

        // Inputs have no values — no condition matches
        assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);

        engine.set_input_value(id, PinId(2), Some(num(30))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1)], "only the first match");

        engine.set_node_property(id, "all_matches", DataValue::Bool(true)).unwrap();
        engine.set_input_value(id, PinId(1), Some(DataValue::String("auto".into()))).unwrap();
        engine.set_input_value(id, PinId(2), Some(num(120))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1), PinId(2), PinId(3)]);

        engine.set_input_value(id, PinId(2), Some(num(10))).unwrap();
        assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);
    }

    #[test]
    fn test_match_rejects_invalid_cases() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[cases(&["5: a > 1", "5: a < 0", "0: a = 3"])]).unwrap();
        // Duplicate pin numbers and the Else pin are rejected
        assert_eq!(engine.node_model(id).unwrap().properties[1], cases(&["5: a > 1"]));
        // Invalid list is not applied
        engine.set_node_property(id, "cases", cases(&["1: a >"]).1).unwrap();
        assert_eq!(engine.node_model(id).unwrap().properties[1], cases(&["5: a > 1"]));
    }

    fn inputs(list: &[&str]) -> (String, DataValue) {
        ("inputs".to_string(), cases(list).1)
    }

    #[test]
    fn test_match_explicit_inputs_and_types() {
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[
            inputs(&["1:now:time", "2:limit:time", "3:list:list", "3:dup:any", "4:list:any"]),
            cases(&["1: now > limit", "2: list contains 3", "3: now >= \"22:00\""]),
        ]).unwrap();
        let def = engine.node_definition(id).unwrap();
        let ins: Vec<_> = def.pins.iter().filter(|p| p.direction == PinDirection::Input).map(|p| (p.id.0, p.name.as_str())).collect();
        // Duplicate pins and names are rejected
        assert_eq!(ins, [(0, "In"), (1, "now"), (2, "limit"), (3, "list")]);
        assert!(def.pins.iter().any(|p| p.id == PinId(1) && p.pin_type == PinType::Data(DataType::Time)));

        let time = |t: &str| DataValue::Time(ticknode_shared::text::parse_time(t).unwrap());
        engine.set_input_value(id, PinId(1), Some(time("22:30"))).unwrap();
        engine.set_input_value(id, PinId(2), Some(time("23:00"))).unwrap();
        // now < limit, list is empty, but now ≥ 22:00 (string is coerced to time)
        assert_eq!(fire(&mut engine, id), [PinId(3)]);
        engine.set_input_value(id, PinId(2), Some(time("21:00"))).unwrap();
        assert_eq!(fire(&mut engine, id), [PinId(1)]);

        // Explicit inputs survive save and load
        let graph = engine.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        assert_eq!(copy.node_definition(id).unwrap().pins, def.pins);
    }

    #[test]
    fn test_match_inputs_are_explicit_after_creation() {
        // A newly created node and the same node after reload react identically to case changes
        let mut engine = TickEngine::new();
        let id = NodeId::generate();
        engine.create_node(id, "match", &[]).unwrap();
        engine.set_node_property(id, "cases", cases(&["1: x > 0"]).1).unwrap();
        let fresh = engine.node_definition(id).unwrap().pins;

        let mut copy = TickEngine::new();
        copy.create_node(id, "match", &[]).unwrap();
        let graph = copy.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        copy.set_node_property(id, "cases", cases(&["1: x > 0"]).1).unwrap();
        assert_eq!(copy.node_definition(id).unwrap().pins, fresh);
        // Input `a` remained, `x` did not appear automatically
        assert!(fresh.iter().any(|p| p.name == "a") && !fresh.iter().any(|p| p.name == "x"));
    }

    #[test]
    fn test_input_values() {
        let mut engine = TickEngine::new();
        let add = NodeId::generate();
        engine.create_node(add, "math/add", &[]).unwrap();
        engine.set_input_value(add, PinId(0), Some(num(2))).unwrap();
        engine.set_input_value(add, PinId(1), Some(num(3))).unwrap();
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // Type must match the pin, and the pin must be a data input
        assert!(engine.set_input_value(add, PinId(0), Some(DataValue::String("x".into()))).is_err());
        assert!(engine.set_input_value(add, PinId(2), Some(num(1))).is_err());

        // A connected source takes precedence over an inline value on the node
        let c = NodeId::generate();
        engine.create_node(c, "const", &[("value".into(), num(10))]).unwrap();
        engine.connect_data(c, PinId(0), add, PinId(0)).unwrap();
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(13)));
        engine.disconnect_data(add, PinId(0));
        assert_eq!(engine.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // Values are saved as properties and restored
        let graph = engine.export_graph();
        let mut copy = TickEngine::new();
        assert!(copy.load_graph(&graph).is_empty());
        assert_eq!(copy.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

        // When a pin disappears after a property change, its value is removed
        let m = NodeId::generate();
        engine.create_node(m, "match", &[cases(&["1: a > b"])]).unwrap();
        engine.set_input_value(m, PinId(2), Some(num(1))).unwrap();
        engine.set_node_property(m, "inputs", inputs(&["1:a:any"]).1).unwrap();
        assert!(engine.node_model(m).unwrap().properties.iter().all(|(n, _)| parse_input_value_property(n).is_none()));
        engine.set_input_value(add, PinId(1), None).unwrap();
        assert_eq!(engine.node_model(add).unwrap().properties, vec![(input_value_property(PinId(0)), num(2))]);
    }

    #[test]
    fn test_every_built_in_node_has_help() {
        let engine = TickEngine::new();
        for def in engine.registry.get_definitions() {
            assert!(!def.help.trim().is_empty(), "node {} has no help", def.type_name);
        }
    }

    #[test]
    fn test_built_in_logic_nodes() {
        let mut engine = TickEngine::new();

        // Match with a single case "a = true" on pin 2 — equivalent to the former Branch node
        let branch_id = NodeId::generate();
        engine.create_node(branch_id, "match", &[cases(&["2: a = true"])]).unwrap();

        // Compare
        let compare_id = NodeId::generate();
        let compare = engine.registry.create_node("compare", compare_id).unwrap();
        engine.add_node(compare);

        // Connect compare inputs to Const nodes
        let const_a_id = NodeId::generate();
        let const_a = Box::new(ConstNode { id: const_a_id, value: DataValue::Number(NumberValue::Int(10)) });
        engine.add_node(const_a);

        let const_b_id = NodeId::generate();
        let const_b = Box::new(ConstNode { id: const_b_id, value: DataValue::Number(NumberValue::Int(5)) });
        engine.add_node(const_b);

        engine.connect_data(const_a_id, PinId(0), compare_id, PinId(0)).unwrap();
        engine.connect_data(const_b_id, PinId(0), compare_id, PinId(1)).unwrap();

        // Pull compare outputs to verify
        let mut stack = Vec::new();
        let eq_val = engine.pull_data(compare_id, PinId(2), &mut stack).unwrap();
        let gt_val = engine.pull_data(compare_id, PinId(3), &mut stack).unwrap();
        let lt_val = engine.pull_data(compare_id, PinId(4), &mut stack).unwrap();

        assert_eq!(eq_val, DataValue::Bool(false));
        assert_eq!(gt_val, DataValue::Bool(true));
        assert_eq!(lt_val, DataValue::Bool(false));

        // Compare Gt → input a of the Match node
        engine.connect_data(compare_id, PinId(3), branch_id, PinId(1)).unwrap();

        // Trigger Match
        engine.trigger_node(branch_id, PinId(0));
        
        // Case 2 → sink
        let target_id = add_sink(&mut engine);
        engine.connect_exec(branch_id, PinId(2), target_id, PinId(0)).unwrap();

        engine.tick();
        // Gt = true, so pin 2 fires and the sink is queued
        assert!(engine.execution_queue.contains(&(target_id, PinId(0))));
    }

    #[test]
    fn test_built_in_for_loop_and_sleep() {
        let mut engine = TickEngine::new();

        // ForLoop
        let loop_id = NodeId::generate();
        let for_loop = engine.registry.create_node("for_loop", loop_id).unwrap();
        engine.add_node(for_loop);

        // Connect Loop Count to Const
        let const_count_id = NodeId::generate();
        let const_count = Box::new(ConstNode { id: const_count_id, value: DataValue::Number(NumberValue::Int(3)) });
        engine.add_node(const_count);
        engine.connect_data(const_count_id, PinId(0), loop_id, PinId(1)).unwrap();

        // Connect LoopBody and Completed to dummy target nodes to trace exec flow
        let dummy_body_id = add_sink(&mut engine);
        let dummy_completed_id = add_sink(&mut engine);
        engine.connect_exec(loop_id, PinId(4), dummy_body_id, PinId(0)).unwrap();
        engine.connect_exec(loop_id, PinId(6), dummy_completed_id, PinId(0)).unwrap();

        // Trigger In (Pin 0)
        engine.trigger_node(loop_id, PinId(0));
        engine.tick();

        // It should output LoopBody (Pin 4) -> should trigger dummy_body
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(0))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Index = 1, LoopBody
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(1))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Index = 2, LoopBody
        assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(2))));
        assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
        engine.execution_queue.clear();

        // Trigger Next (Pin 3)
        engine.trigger_node(loop_id, PinId(3));
        engine.tick();
        // Completed (Pin 6) -> should trigger dummy_completed
        assert!(engine.execution_queue.contains(&(dummy_completed_id, PinId(0))));
        engine.execution_queue.clear();

        // --- Sleep Node ---
        let sleep_id = NodeId::generate();
        let sleep_node = engine.registry.create_node("sleep", sleep_id).unwrap();
        engine.add_node(sleep_node);

        // Connect Sleep inputs
        let const_delay_id = NodeId::generate();
        let const_delay = Box::new(ConstNode { id: const_delay_id, value: DataValue::Number(NumberValue::Int(3)) });
        engine.add_node(const_delay);
        engine.connect_data(const_delay_id, PinId(0), sleep_id, PinId(1)).unwrap();

        let const_ticks_id = NodeId::generate();
        let const_ticks = Box::new(ConstNode { id: const_ticks_id, value: DataValue::Bool(true) });
        engine.add_node(const_ticks);
        engine.connect_data(const_ticks_id, PinId(0), sleep_id, PinId(2)).unwrap();

        // Connect Sleep Out to dummy target node to trace exec flow
        let dummy_sleep_out_id = add_sink(&mut engine);
        engine.connect_exec(sleep_id, PinId(4), dummy_sleep_out_id, PinId(0)).unwrap();

        engine.trigger_node(sleep_id, PinId(0)); // trigger In
        engine.tick(); // task created

        assert_eq!(engine.host_state.active_tasks.len(), 1);
        let task_handle = *engine.host_state.active_tasks.keys().next().unwrap();

        engine.tick(); // elapsed = 1
        assert!(engine.host_state.active_tasks.contains_key(&task_handle));

        engine.tick(); // elapsed = 2
        assert!(engine.host_state.active_tasks.contains_key(&task_handle));

        engine.tick(); // elapsed = 3 -> task finished and delivered to the node, Out queues the next node
        assert!(!engine.host_state.active_tasks.contains_key(&task_handle));
        assert!(engine.execution_queue.contains(&(dummy_sleep_out_id, PinId(0))));
    }

    #[test]
    fn test_built_in_collections() {
        let mut engine = TickEngine::new();

        // 1. List
        let create_id = NodeId::generate();
        let mut create_node = engine.registry.create_node("list/create", create_id).unwrap();
        create_node.set_property("element_type", DataValue::String("number".to_string()));
        engine.add_node(create_node);

        let add_id = NodeId::generate();
        let mut add_node = engine.registry.create_node("list/add", add_id).unwrap();
        add_node.set_property("element_type", DataValue::String("number".to_string()));
        engine.add_node(add_node);

        // Connect List from create to add
        engine.connect_data(create_id, PinId(0), add_id, PinId(0)).unwrap();

        // Connect Item to Const
        let const_item_id = NodeId::generate();
        let const_item = Box::new(ConstNode { id: const_item_id, value: DataValue::Number(NumberValue::Int(42)) });
        engine.add_node(const_item);
        engine.connect_data(const_item_id, PinId(0), add_id, PinId(1)).unwrap();

        // Pull output of add_node
        let mut stack = Vec::new();
        let res_list = engine.pull_data(add_id, PinId(2), &mut stack).unwrap();
        if let DataValue::List { element_type, elements } = res_list {
            assert_eq!(element_type, DataType::Number);
            assert_eq!(elements, vec![DataValue::Number(NumberValue::Int(42))]);
        } else {
            panic!("Expected list output");
        }

        // 2. Object
        let obj_create_id = NodeId::generate();
        let obj_create = engine.registry.create_node("object/create", obj_create_id).unwrap();
        engine.add_node(obj_create);

        let obj_set_id = NodeId::generate();
        let mut obj_set = engine.registry.create_node("object/set_field", obj_set_id).unwrap();
        obj_set.set_property("field_type", DataValue::String("number".to_string()));
        engine.add_node(obj_set);

        // Connect obj_create to obj_set
        engine.connect_data(obj_create_id, PinId(0), obj_set_id, PinId(0)).unwrap();

        // Connect Name and Val to Const nodes
        let const_name_id = NodeId::generate();
        let const_name = Box::new(ConstNode { id: const_name_id, value: DataValue::String("age".to_string()) });
        engine.add_node(const_name);
        engine.connect_data(const_name_id, PinId(0), obj_set_id, PinId(1)).unwrap();

        let const_val_id = NodeId::generate();
        let const_val = Box::new(ConstNode { id: const_val_id, value: DataValue::Number(NumberValue::Int(30)) });
        engine.add_node(const_val);
        engine.connect_data(const_val_id, PinId(0), obj_set_id, PinId(2)).unwrap();

        // Pull object
        stack.clear();
        let res_obj = engine.pull_data(obj_set_id, PinId(3), &mut stack).unwrap();

        // Now Unpack
        let unpack_id = NodeId::generate();
        let mut unpack = engine.registry.create_node("object/unpack", unpack_id).unwrap();
        unpack.set_property("fields_list", DataValue::String("age:number,name:string".to_string()));
        engine.add_node(unpack);

        // Set unpack Obj input by creating a Const node wrapper for res_obj
        let const_obj_id = NodeId::generate();
        let const_obj = Box::new(ConstNode { id: const_obj_id, value: res_obj });
        engine.add_node(const_obj);
        engine.connect_data(const_obj_id, PinId(0), unpack_id, PinId(0)).unwrap();

        // Pull age (pin 1) and name (pin 2)
        stack.clear();
        let age_val = engine.pull_data(unpack_id, PinId(1), &mut stack).unwrap();
        let name_val = engine.pull_data(unpack_id, PinId(2), &mut stack); // name is missing, should be None

        assert_eq!(age_val, DataValue::Number(NumberValue::Int(30)));
        assert!(name_val.is_none());
    }

    #[test]
    fn test_click_button_and_debug_nodes() {
        let mut engine = TickEngine::new();

        // 1. Create ClickButton
        let button_id = NodeId::generate();
        let button_node = engine.registry.create_node("test/click_button", button_id).unwrap();
        engine.add_node(button_node);

        // 2. Create DebugNode and set prefix
        let debug_id = NodeId::generate();
        let mut debug_node = engine.registry.create_node("test/debug", debug_id).unwrap();
        debug_node.set_property("prefix", DataValue::String("variable value: ".to_string()));
        engine.add_node(debug_node);

        // 3. Connect Exec: Button Out -> Debug In
        engine.connect_exec(button_id, PinId(0), debug_id, PinId(0)).unwrap();

        // 4. Create ConstNode (444) and connect to Debug Val (Any)
        let const_id = NodeId::generate();
        let const_node = Box::new(ConstNode { id: const_id, value: DataValue::Number(NumberValue::Int(444)) });
        engine.add_node(const_node);
        engine.connect_data(const_id, PinId(0), debug_id, PinId(1)).unwrap(); // Any connection must succeed!

        // Simulate user click
        engine.set_node_property(button_id, "clicked", DataValue::Bool(true)).unwrap();
        engine.trigger_node(button_id, PinId(0));

        // First tick: Button executes, debug node is queued for the next tick
        engine.tick();
        assert!(engine.execution_queue.contains(&(debug_id, PinId(0))));

        // Second tick: Debug node executes
        engine.tick();
        assert!(engine.execution_queue.is_empty());
    }

    #[test]
    fn test_graph_roundtrip_keeps_properties_and_connections() {
        let mut engine = TickEngine::new();
        let a = NodeId::generate();
        let b = NodeId::generate();
        let dbg = NodeId::generate();
        engine.create_node(a, "const", &[("value".into(), DataValue::String("hi".into()))]).unwrap();
        engine.create_node(b, "math/expression", &[("expression".into(), DataValue::String("x * 2".into()))]).unwrap();
        engine.create_node(dbg, "test/debug", &[("prefix".into(), DataValue::String("> ".into()))]).unwrap();
        engine.connect_data(a, PinId(0), dbg, PinId(1)).unwrap();

        let graph = engine.export_graph();
        let mut restored = TickEngine::new();
        let warnings = restored.load_graph(&graph);
        assert!(warnings.is_empty(), "{:?}", warnings);
        assert_eq!(restored.export_graph(), graph);
        // Formula pin was restored from the expression
        let def = restored.node_definition(b).unwrap();
        assert!(def.pins.iter().any(|p| p.name == "x" && p.direction == PinDirection::Input));
    }

    #[test]
    fn test_data_input_has_single_source_and_exec_is_validated() {
        let mut engine = TickEngine::new();
        let c1 = NodeId::generate();
        let c2 = NodeId::generate();
        let dbg = add_sink(&mut engine);
        engine.create_node(c1, "const", &[]).unwrap();
        engine.create_node(c2, "const", &[]).unwrap();
        engine.connect_data(c1, PinId(0), dbg, PinId(1)).unwrap();
        engine.connect_data(c2, PinId(0), dbg, PinId(1)).unwrap();
        assert_eq!(engine.data_connections().len(), 1);
        assert_eq!(engine.data_connections()[0].src_node, c2);

        // Data pin cannot be connected as exec
        assert_eq!(engine.connect_exec(c1, PinId(0), dbg, PinId(0)), Err(EngineError::InvalidPinConnection));
    }

    #[test]
    fn test_property_change_prunes_invalid_connections() {
        let mut engine = TickEngine::new();
        let x = NodeId::generate();
        let expr = NodeId::generate();
        engine.create_node(x, "const", &[]).unwrap();
        engine.create_node(expr, "math/expression", &[("expression".into(), DataValue::String("a + b".into()))]).unwrap();
        engine.connect_data(x, PinId(0), expr, PinId(2)).unwrap(); // b

        // Formula now has only one variable — pin 2 no longer exists
        let removed = engine.set_node_property(expr, "expression", DataValue::String("a * 3".into())).unwrap();
        assert_eq!(removed.data.len(), 1);
        assert!(engine.data_connections().is_empty());
    }

    #[test]
    fn test_remove_node_cancels_its_tasks() {
        let mut engine = TickEngine::new();
        let sleep = NodeId::generate();
        engine.create_node(sleep, "sleep", &[]).unwrap();
        let delay = NodeId::generate();
        engine.create_node(delay, "const", &[("value".into(), DataValue::Number(NumberValue::Int(100)))]).unwrap();
        engine.connect_data(delay, PinId(0), sleep, PinId(1)).unwrap();
        engine.trigger_node(sleep, PinId(0));
        engine.tick();
        assert_eq!(engine.host_state.active_tasks.len(), 1);

        engine.remove_node(sleep);
        assert!(engine.host_state.active_tasks.is_empty());
    }

    #[test]
    fn test_clock_fires_every_interval() {
        let mut engine = TickEngine::new();
        let clock = NodeId::generate();
        engine.create_node(clock, "clock", &[("interval_ticks".into(), DataValue::Number(NumberValue::Int(3)))]).unwrap();
        let sink = add_sink(&mut engine);
        engine.connect_exec(clock, PinId(0), sink, PinId(0)).unwrap();

        let mut fired_on = Vec::new();
        for _ in 0..9 {
            engine.tick();
            if engine.executed.iter().any(|t| t.node_id == clock) {
                fired_on.push(engine.host_state.current_tick);
            }
        }
        assert_eq!(fired_on, vec![3, 6, 9]);
    }

    #[test]
    fn test_traces_record_inputs_outputs_and_fired_pins() {
        let mut engine = TickEngine::new();
        let num = |v| DataValue::Number(NumberValue::Int(v));
        let (a, b, add, debug) = (NodeId::generate(), NodeId::generate(), NodeId::generate(), NodeId::generate());
        engine.create_node(a, "const", &[("value".into(), num(2))]).unwrap();
        engine.create_node(b, "const", &[("value".into(), num(3))]).unwrap();
        engine.create_node(add, "math/add", &[]).unwrap();
        engine.create_node(debug, "test/debug", &[]).unwrap();
        engine.connect_data(a, PinId(0), add, PinId(0)).unwrap();
        engine.connect_data(b, PinId(0), add, PinId(1)).unwrap();
        engine.connect_data(add, PinId(2), debug, PinId(1)).unwrap();

        engine.trigger_node(debug, PinId(0));
        engine.tick();

        let trace = |id| engine.executed.iter().find(|t| t.node_id == id).cloned().unwrap();
        let d = trace(debug);
        assert_eq!(d.tick, 1);
        assert_eq!(d.inputs, vec![(PinId(1), Some(num(5)))]);
        assert_eq!(d.fired, vec![PinId(2)]);
        // Pure node evaluated on demand also produces a trace.
        let s = trace(add);
        assert_eq!(s.inputs, vec![(PinId(0), Some(num(2))), (PinId(1), Some(num(3)))]);
        assert_eq!(s.outputs, vec![(PinId(2), num(5))]);

        engine.tick();
        assert!(engine.executed.is_empty());
    }

    #[test]
    fn test_import_snippet_maps_ids_and_keeps_inner_connections() {
        let mut engine = TickEngine::new();
        let (a, b, outside) = (NodeId::generate(), NodeId::generate(), NodeId::generate());
        engine.create_node(a, "const", &[]).unwrap();
        engine.create_node(b, "test/debug", &[]).unwrap();
        engine.create_node(outside, "test/debug", &[]).unwrap();
        engine.connect_data(a, PinId(0), b, PinId(1)).unwrap();
        engine.connect_data(a, PinId(0), outside, PinId(1)).unwrap();
        let mut snippet = engine.export_snippet(&[a, b]);
        // Connection to a node outside the snippet (e.g., from another graph's clipboard) is ignored.
        snippet.data_connections.push(ConnectionModel { src_node: a, src_pin: PinId(0), dst_node: outside, dst_pin: PinId(1) });

        let (created, warnings) = engine.import_snippet(&snippet);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(created.iter().map(|(old, _)| *old).collect::<Vec<_>>(), vec![a, b]);
        let (new_a, new_b) = (created[0].1, created[1].1);
        assert!(new_a != a && new_b != b);
        assert_eq!(engine.export_snippet(&[new_a, new_b]).data_connections.len(), 1);
        assert_eq!(engine.node_ids().len(), 5);
    }
}