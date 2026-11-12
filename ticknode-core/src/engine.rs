use crate::{HostState, Node, NodeRegistry, VariableStore, plugin};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use ticknode_shared::*;

mod connections;
mod connections_index;
#[cfg(test)]
mod disabled_tests;
mod errors;
mod execution;
mod frozen;
mod nodes;
mod persistence;
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
    node_errors: errors::NodeErrors,
    successful: std::collections::HashSet<NodeId>,
    /// Work accounting for the most recent tick, including tasks, emissions, polls and pure pulls.
    pub last_tick: crate::TickStats,
    pub(crate) confirmed_pins: HashMap<NodeId, Vec<PinSignature>>,
    pub(crate) frozen: HashMap<NodeId, Vec<PinChange>>,
    pub(crate) frozen_reported: std::collections::HashSet<NodeId>,
    pub(crate) disabled: std::collections::HashSet<NodeId>,
    pub(crate) detached: Vec<DetachedConnection>,
    pub(crate) nodes: HashMap<NodeId, Box<dyn Node>>,
    definitions: HashMap<NodeId, (u64, Arc<NodeDefinition>)>,
    /// Event sources (`Node::polls`), polled every tick.
    pub(crate) sources: BTreeSet<NodeId>,
    /// Nodes whose `ensure_live` must run every tick (`Node::needs_live`).
    pub(crate) live_nodes: BTreeSet<NodeId>,
    pub(crate) connections: ConnectionIndex,
    pub registry: NodeRegistry,
    /// Core quantities and custom data types declared by plugins.
    pub types: TypeRegistry,
    /// Which plugin declared each custom data type.
    pub(crate) type_owners: HashMap<String, String>,
    /// Loaded plugins (both enabled and disabled); see `plugin.rs`.
    pub(crate) plugins: std::collections::BTreeMap<String, plugin::PluginSlot>,
    pub data_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub pure_cache: HashMap<NodeId, HashMap<PinId, DataValue>>,
    pub variables: VariableStore,
    pub node_sheets: HashMap<NodeId, SheetId>,
    pub current_tick_changed_variables: std::collections::HashSet<VariableKey>,
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
    /// Position of each node's trace in `executed`; verified on use because `executed` is public.
    executed_index: HashMap<NodeId, usize>,
}

impl Default for TickEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TickEngine {
    pub fn new() -> Self {
        Self {
            budget: Default::default(),
            node_errors: Default::default(),
            successful: Default::default(),
            last_tick: Default::default(),
            confirmed_pins: HashMap::new(),
            frozen: HashMap::new(),
            frozen_reported: Default::default(),
            disabled: std::collections::HashSet::new(),
            detached: Vec::new(),
            nodes: HashMap::new(),
            definitions: HashMap::new(),
            sources: BTreeSet::new(),
            live_nodes: BTreeSet::new(),
            connections: ConnectionIndex::default(),
            registry: NodeRegistry::new(),
            types: TypeRegistry::from_definitions(quantity::builtin_quantities()),
            type_owners: HashMap::new(),
            plugins: Default::default(),
            data_cache: HashMap::new(),
            pure_cache: HashMap::new(),
            variables: VariableStore::new(),
            node_sheets: HashMap::new(),
            current_tick_changed_variables: std::collections::HashSet::new(),
            input_values: HashMap::new(),
            split_pins: HashMap::new(),
            required_reported: Default::default(),
            execution_queue: VecDeque::new(),
            host_state: HostState::default(),
            executed: Vec::new(),
            executed_index: HashMap::new(),
        }
    }

    /// Changes server text formatting and invalidates cached pure results immediately.
    pub fn set_locale(&mut self, locale: &str) {
        self.host_state.locale = if locale == "uk" { "uk" } else { "en" }.into();
        self.pure_cache.clear();
    }

    pub fn set_display_rules(&mut self, rules: ticknode_shared::DisplayRules) {
        self.host_state.display = rules;
        self.pure_cache.clear();
    }

    pub fn set_node_sheet(&mut self, node: NodeId, sheet: Option<SheetId>) {
        if let Some(s) = sheet {
            self.node_sheets.insert(node, s);
        } else {
            self.node_sheets.remove(&node);
        }
        if let Some(n) = self.nodes.get_mut(&node) {
            n.set_sheet(sheet);
        }
    }
}
