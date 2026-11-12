use crate::{EngineError, StubNode, TickEngine};
use std::collections::HashMap;
use ticknode_shared::*;

impl TickEngine {
    /// Exports in a deterministic order (friendly for git diffs).
    pub fn export_graph(&self) -> GraphModel {
        let mut ids = self.node_ids();
        ids.sort_by_key(|id| id.0);
        self.export_snippet(&ids)
    }

    pub fn export_snippet(&self, node_ids: &[NodeId]) -> GraphModel {
        let nodes = node_ids
            .iter()
            .filter_map(|id| self.node_model(*id))
            .collect();
        let sort_key = |c: &ConnectionModel| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0);
        let mut exec_connections: Vec<_> = self
            .connections
            .exec()
            .iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied()
            .collect();
        exec_connections.sort_by_key(sort_key);
        let mut data_connections: Vec<_> = self
            .connections
            .data()
            .iter()
            .filter(|c| node_ids.contains(&c.src_node) && node_ids.contains(&c.dst_node))
            .copied()
            .collect();
        data_connections.sort_by_key(sort_key);
        let mut detached_connections: Vec<_> = self
            .detached
            .iter()
            .filter(|d| {
                node_ids.contains(&d.connection.src_node)
                    && node_ids.contains(&d.connection.dst_node)
            })
            .copied()
            .collect();
        detached_connections.sort_by_key(|d| {
            (
                d.exec,
                sort_key(&d.connection),
                d.end == DetachedEnd::Target,
                d.row,
            )
        });
        GraphModel {
            nodes,
            exec_connections,
            data_connections,
            wire_routes: Vec::new(),
            detached_connections,
            frames: Vec::new(),
        }
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
        let id_map: HashMap<NodeId, NodeId> = snippet
            .nodes
            .iter()
            .map(|n| (n.id, NodeId::generate()))
            .collect();
        let inside = |c: &&ConnectionModel| {
            id_map.contains_key(&c.src_node) && id_map.contains_key(&c.dst_node)
        };
        let snippet = GraphModel {
            nodes: snippet.nodes.clone(),
            exec_connections: snippet
                .exec_connections
                .iter()
                .filter(inside)
                .copied()
                .collect(),
            data_connections: snippet
                .data_connections
                .iter()
                .filter(inside)
                .copied()
                .collect(),
            wire_routes: Vec::new(),
            detached_connections: snippet
                .detached_connections
                .iter()
                .filter(|d| inside(&&d.connection))
                .copied()
                .collect(),
            frames: Vec::new(),
        };
        let warnings = self.insert_graph(&snippet, |id| id_map[&id]);
        let created = snippet
            .nodes
            .iter()
            .map(|n| (n.id, id_map[&n.id]))
            .filter(|(_, new)| self.has_node(*new))
            .collect();
        (created, warnings)
    }

    pub(super) fn insert_graph(
        &mut self,
        graph: &GraphModel,
        map_id: impl Fn(NodeId) -> NodeId,
    ) -> Vec<String> {
        self.insert_graph_edit(graph, map_id, None, None)
    }

    /// `user_edit` is the node a user command recreated (its broken connections are removed, not
    /// detached); `accept` is that node when it may accept its current pins without freezing.
    pub(super) fn insert_graph_edit(
        &mut self,
        graph: &GraphModel,
        map_id: impl Fn(NodeId) -> NodeId,
        user_edit: Option<NodeId>,
        accept: Option<NodeId>,
    ) -> Vec<String> {
        let mut warnings = Vec::new();
        let mut unknown = Vec::new();
        for n in &graph.nodes {
            match self.create_node(map_id(n.id), &n.type_name, &n.properties) {
                Ok(()) => {}
                Err(EngineError::UnknownNodeType(_)) => unknown.push(n),
                Err(e) => warnings.push(format!("Node {:?}: {}", n.id, e)),
            }
        }
        self.insert_unknown_nodes(unknown, graph, &map_id);
        for n in &graph.nodes {
            let id = map_id(n.id);
            self.restore_pins(
                id,
                if accept == Some(id) {
                    None
                } else {
                    n.pins.as_deref()
                },
            );
        }
        let user_connection =
            |c: &ConnectionModel| user_edit.is_some_and(|id| c.src_node == id || c.dst_node == id);
        for c in &graph.exec_connections {
            if let Err(e) =
                self.connect_exec(map_id(c.src_node), c.src_pin, map_id(c.dst_node), c.dst_pin)
            {
                let mapped = ConnectionModel {
                    src_node: map_id(c.src_node),
                    dst_node: map_id(c.dst_node),
                    ..*c
                };
                if user_connection(&mapped) || !self.detach_invalid(true, mapped, &e) {
                    warnings.push(format!("Exec connection {:?}: {}", c, e));
                }
            }
        }
        for c in &graph.data_connections {
            if let Err(e) = self.connect_saved_data(ConnectionModel {
                src_node: map_id(c.src_node),
                dst_node: map_id(c.dst_node),
                ..*c
            }) {
                let mapped = ConnectionModel {
                    src_node: map_id(c.src_node),
                    dst_node: map_id(c.dst_node),
                    ..*c
                };
                if user_connection(&mapped) || !self.detach_invalid(false, mapped, &e) {
                    warnings.push(format!("Data connection {:?}: {}", c, e));
                }
            }
        }
        let detached: Vec<_> = graph
            .detached_connections
            .iter()
            .map(|d| DetachedConnection {
                connection: ConnectionModel {
                    src_node: map_id(d.connection.src_node),
                    dst_node: map_id(d.connection.dst_node),
                    ..d.connection
                },
                ..*d
            })
            .collect();
        self.restore_detached(&detached);
        warnings
    }
    fn insert_unknown_nodes(
        &mut self,
        unknown: Vec<&NodeModel>,
        graph: &GraphModel,
        map_id: &impl Fn(NodeId) -> NodeId,
    ) {
        // An unknown type (plugin disabled or not loaded) does not lose the node: a stub preserves
        // properties and connections so the graph saves back without data loss
        for n in unknown {
            let id = map_id(n.id);
            let mut pins = self.infer_stub_pins(id, graph, map_id);
            for p in n.pins.iter().flatten() {
                let split = n.properties.iter().any(|(key, value)| {
                    parse_split_pin_property(key) == Some(p.id) && *value == DataValue::Bool(true)
                });
                if split {
                    continue;
                }
                pins.retain(|pin| pin.id != p.id || pin.direction != p.direction);
                pins.push(PinDefinition {
                    id: p.id,
                    name: p.name.clone(),
                    direction: p.direction,
                    pin_type: p.kind.clone(),
                    required: p.required,
                });
            }
            tracing::warn!(node = ?n.id, "Unknown node type {}: created a stub", n.type_name);
            self.add_node(Box::new(StubNode::new(
                id,
                StubNode::bare_definition(&n.type_name, pins),
                n.properties.clone(),
                None,
            )));
        }
    }
}
