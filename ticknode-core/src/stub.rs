//! Stub nodes: a node whose type is currently unavailable (plugin disabled or not loaded).
//!
//! A stub retains its type, all properties (including `in:*` input values), and its last known
//! definition (pins), so connections remain intact and the graph saves to disk unchanged. It does not
//! execute or accept modifications; when the type reappears in the registry, a real node is
//! created in its place from the same properties.

use std::collections::HashSet;

use ticknode_shared::*;

use crate::{EngineError, Node, NodeContext, RemovedConnections, TickEngine};

pub struct StubNode {
    id: NodeId,
    definition: NodeDefinition,
    properties: Vec<(String, DataValue)>,
    /// The disabled plugin; `None` means the type is simply unknown (plugin not loaded).
    plugin: Option<String>,
}

impl StubNode {
    pub fn new(id: NodeId, definition: NodeDefinition, properties: Vec<(String, DataValue)>, plugin: Option<String>) -> Self {
        Self { id, definition, properties, plugin }
    }

    /// Definition for a type whose pins are known only from graph connections.
    pub fn bare_definition(type_name: &str, pins: Vec<PinDefinition>) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: type_name.into(),
            title: type_name.into(),
            category: "Unavailable".into(),
            description: "Node type unavailable".into(),
            help: String::new(),
            pins,
        }
    }

    pub fn message(&self) -> UiMessage {
        match &self.plugin {
            Some(plugin) => UiMessage::new("error-node-plugin-unavailable", format!("plugin {plugin} unavailable")).arg("plugin", plugin),
            None => UiMessage::new(
                "error-node-type-unknown",
                format!("node type {} is unknown: the required plugin is missing or disabled", self.definition.type_name),
            ).arg("type", &self.definition.type_name),
        }
    }
}

impl Node for StubNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition { self.definition.clone() }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        ctx.host_state.error(self.id, self.message(), format!("[ERROR] Node is not executed: {}", self.message()));
        Vec::new()
    }

    fn properties(&self) -> Vec<(String, DataValue)> { self.properties.clone() }
    fn unavailable(&self) -> Option<UiMessage> { Some(self.message()) }
}

impl TickEngine {
    /// Stub pins for an unknown type: inferred from graph connections. A data pin's type is taken
    /// from the other end of the connection, or `Any` if the other end is also a stub.
    pub(crate) fn infer_stub_pins(&self, id: NodeId, graph: &GraphModel, map_id: &impl Fn(NodeId) -> NodeId) -> Vec<PinDefinition> {
        let mut pins: Vec<PinDefinition> = Vec::new();
        let other_type = |node: NodeId, pin: PinId, direction: PinDirection| {
            self.node_definition(node)
                .and_then(|d| d.pins.into_iter().find(|p| p.id == pin && p.direction == direction))
                .map_or(PinType::Data(DataType::Any), |p| p.pin_type)
        };
        for c in &graph.exec_connections {
            if map_id(c.src_node) == id {
                add_stub_pin(&mut pins, c.src_pin, PinDirection::Output, PinType::Exec);
            }
            if map_id(c.dst_node) == id {
                add_stub_pin(&mut pins, c.dst_pin, PinDirection::Input, PinType::Exec);
            }
        }
        for c in &graph.data_connections {
            if map_id(c.src_node) == id {
                add_stub_pin(&mut pins, c.src_pin, PinDirection::Output, other_type(map_id(c.dst_node), c.dst_pin, PinDirection::Input));
            }
            if map_id(c.dst_node) == id {
                add_stub_pin(&mut pins, c.dst_pin, PinDirection::Input, other_type(map_id(c.src_node), c.src_pin, PinDirection::Output));
            }
        }
        pins.sort_by_key(|p| (p.direction == PinDirection::Output, p.id.0));
        pins
    }

    /// Replaces nodes of the given types with stubs (plugin disabled). Returns the IDs of replaced nodes.
    pub(crate) fn replace_with_stubs(&mut self, type_names: &HashSet<String>, plugin: &str) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.iter()
            .filter(|(_, n)| type_names.contains(&n.definition().type_name))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_by_key(|id| id.0);
        for id in &ids {
            let Some(model) = self.node_model(*id) else { continue };
            let definition = self.node_definition(*id).expect("node exists");
            self.host_state.cancel_node_tasks(*id);
            self.input_values.remove(id);
            self.split_pins.remove(id);
            self.required_reported.remove(id);
            self.data_cache.remove(id);
            self.pure_cache.remove(id);
            // Old node (WasmNode) drops its plugin resource when replaced
            self.add_node(Box::new(StubNode::new(*id, definition, model.properties, Some(plugin.to_string()))));
        }
        self.pure_cache.clear();
        ids
    }

    /// Revives registered types, comparing their confirmed contracts and detaching invalid wires.
    pub(crate) fn revive_stubs(&mut self) -> (Vec<NodeId>, RemovedConnections, Vec<DetachedConnection>) {
        let mut ids: Vec<NodeId> = self.nodes.iter()
            .filter(|(_, n)| n.unavailable().is_some() && self.registry.get_definition(&n.definition().type_name).is_some())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_by_key(|id| id.0);
        let mut revived = Vec::new();
        for id in ids {
            let Some(model) = self.node_model(id) else { continue };
            match self.create_node(id, &model.type_name, &model.properties) {
                Ok(()) => {
                    self.restore_pins(id, model.pins.as_deref());
                    revived.push(id);
                }
                Err(EngineError::UnknownNodeType(_)) => {}
                Err(e) => tracing::warn!(node = ?id, "Failed to restore node {}: {e}", model.type_name),
            }
        }
        self.pure_cache.clear();
        let removed = self.prune_connections(|c| revived.contains(&c.src_node) || revived.contains(&c.dst_node));
        let detached = self.detach_pruned(removed);
        (revived, RemovedConnections::default(), detached)
    }
}

fn add_stub_pin(pins: &mut Vec<PinDefinition>, pin: PinId, direction: PinDirection, pin_type: PinType) {
    if !pins.iter().any(|p| p.id == pin && p.direction == direction) {
        pins.push(PinDefinition { id: pin, name: format!("#{}", pin.0), pin_type, direction, required: false });
    }
}
