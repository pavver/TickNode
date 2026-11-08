use std::collections::HashMap;
use ticknode_shared::*;
use crate::{EngineError, StubNode, TickEngine};

impl TickEngine {
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
}
