use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use ticknode_shared::*;
use crate::{HostState, Node, NodeRegistry, plugin};

mod nodes;
mod connections;
mod connections_index;
mod persistence;
mod execution;
mod reconcile;

pub use reconcile::GraphReconcile;

pub(crate) use connections_index::ConnectionIndex;
pub(crate) use nodes::input_pin_type;

/// Connections removed automatically by the engine (e.g., after a node's pins changed).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct RemovedConnections {
    pub exec: Vec<ConnectionModel>,
    pub data: Vec<ConnectionModel>,
}

pub struct TickEngine {
    /// Scheduling policy consulted before queued activations; unlimited by default.
    pub budget: crate::TickBudget,
    /// Work accounting for the most recent tick, including tasks, emissions, polls and pure pulls.
    pub last_tick: crate::TickStats,
    pub(crate) nodes: HashMap<NodeId, Box<dyn Node>>,
    definitions: HashMap<NodeId, (u64, Arc<NodeDefinition>)>,
    pub(crate) connections: ConnectionIndex,
    pub registry: NodeRegistry,
    /// Custom data types declared by plugins.
    pub types: TypeRegistry,
    /// Which plugin declared each custom data type.
    pub(crate) type_owners: HashMap<String, String>,
    /// Loaded plugins (both enabled and disabled); see `plugin.rs`.
    pub(crate) plugins: std::collections::BTreeMap<String, plugin::PluginSlot>,
    pub data_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub pure_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub variables: HashMap<String, DataValue>,
    /// Values of unconnected inputs set in the editor (`in:<pin>` properties).
    pub(crate) input_values: HashMap<NodeId, BTreeMap<u32, DataValue>>,
    pub(crate) split_pins: HashMap<NodeId, std::collections::BTreeSet<u32>>,
    /// Nodes for which a "missing required input" error has already been logged (to avoid repeating every tick).
    pub(crate) required_reported: std::collections::HashSet<NodeId>,
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
            budget: Default::default(),
            last_tick: Default::default(),
            nodes: HashMap::new(),
            definitions: HashMap::new(),
            connections: ConnectionIndex::default(),
            registry: NodeRegistry::new(),
            types: TypeRegistry::new(),
            type_owners: HashMap::new(),
            plugins: Default::default(),
            data_cache: HashMap::new(),
            pure_cache: HashMap::new(),
            variables: HashMap::new(),
            input_values: HashMap::new(),
            split_pins: HashMap::new(),
            required_reported: Default::default(),
            execution_queue: VecDeque::new(),
            host_state: HostState::default(),
            executed: Vec::new(),
        }
    }
}
