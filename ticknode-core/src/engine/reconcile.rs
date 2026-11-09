use std::collections::{HashMap, HashSet};
use ticknode_shared::*;
use crate::{RemovedConnections, TickEngine};

/// Changes actually made while restoring a graph without resetting untouched nodes.
#[derive(Debug, Default)]
pub struct GraphReconcile {
    pub removed_nodes: Vec<NodeId>,
    pub added_nodes: Vec<NodeId>,
    pub updated_nodes: Vec<NodeId>,
    pub disconnected: RemovedConnections,
    pub connected_exec: Vec<ConnectionModel>,
    pub connected_data: Vec<ConnectionModel>,
    pub warnings: Vec<String>,
}

struct GraphDiff {
    removed_nodes: Vec<NodeId>,
    disconnected: RemovedConnections,
    insert: GraphModel,
}

/// Pure graph comparison. Positions belong to the actor and never recreate engine nodes.
fn diff(current: &GraphModel, target: &GraphModel) -> GraphDiff {
    let before: HashMap<_, _> = current.nodes.iter().map(|n| (n.id, n)).collect();
    let after: HashMap<_, _> = target.nodes.iter().map(|n| (n.id, n)).collect();
    let same = |a: &NodeModel, b: &NodeModel| a.type_name == b.type_name && a.properties == b.properties;
    let kept: HashSet<_> = before.iter().filter_map(|(id, n)| after.get(id).filter(|t| same(n, t)).map(|_| *id)).collect();
    let removed_nodes = current.nodes.iter().filter(|n| !kept.contains(&n.id)).map(|n| n.id).collect();
    let touching = |c: &ConnectionModel| !kept.contains(&c.src_node) || !kept.contains(&c.dst_node);
    let changes = |old: &[ConnectionModel], new: &[ConnectionModel]| {
        let old_set: HashSet<_> = old.iter().copied().collect();
        let new_set: HashSet<_> = new.iter().copied().collect();
        let drop = old.iter().filter(|c| touching(c) || !new_set.contains(c)).copied().collect();
        let add = new.iter().filter(|c| touching(c) || !old_set.contains(c)).copied().collect();
        (drop, add)
    };
    let (exec, exec_connections) = changes(&current.exec_connections, &target.exec_connections);
    let (data, data_connections) = changes(&current.data_connections, &target.data_connections);
    GraphDiff {
        removed_nodes, disconnected: RemovedConnections { exec, data },
        insert: GraphModel {
            nodes: target.nodes.iter().filter(|n| !kept.contains(&n.id)).cloned().collect(),
            exec_connections, data_connections, wire_routes: Vec::new(), detached_connections: Vec::new(), frames: Vec::new(),
        },
    }
}

impl TickEngine {
    /// Restores a snapshot with minimal edits. Unchanged nodes retain tasks and runtime state;
    /// variables and the execution queue are not reset (removed nodes still cancel their work).
    pub fn reconcile(&mut self, target: &GraphModel) -> GraphReconcile {
        self.reconcile_edit(target, None)
    }

    /// Settings pasted by the user accept their new contract and prune invalid wires.
    pub fn reconcile_settings(&mut self, target: &GraphModel, edited: NodeId) -> GraphReconcile {
        self.clear_node_error(edited);
        self.reconcile_edit(target, Some(edited))
    }

    fn reconcile_edit(&mut self, target: &GraphModel, user_edit: Option<NodeId>) -> GraphReconcile {
        let before_frozen = self.frozen.clone();
        let plan = diff(&self.export_graph(), target);
        let mut result = GraphReconcile::default();
        for id in plan.removed_nodes {
            let removed = self.remove_node(id);
            result.disconnected.exec.extend(removed.exec);
            result.disconnected.data.extend(removed.data);
            result.removed_nodes.push(id);
        }
        for c in plan.disconnected.exec {
            if self.disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin) {
                result.disconnected.exec.push(c);
            }
        }
        for c in plan.disconnected.data {
            if self.disconnect_data(c.dst_node, c.dst_pin) {
                result.disconnected.data.push(c);
            }
        }
        self.detached.clear();
        // A user edit accepts new pins only on a node that was not frozen: editing a frozen node
        // keeps it frozen until it is confirmed explicitly.
        let accept = user_edit.filter(|id| !before_frozen.contains_key(id));
        result.warnings = self.insert_graph_edit(&plan.insert, |id| id, user_edit, accept);
        for n in &target.nodes {
            self.restore_pins(n.id, if accept == Some(n.id) { None } else { n.pins.as_deref() });
            if before_frozen.get(&n.id).map(Vec::as_slice).unwrap_or(&[]) != self.frozen_changes(n.id) {
                result.updated_nodes.push(n.id);
            }
        }
        self.restore_detached(&target.detached_connections);
        result.added_nodes = plan.insert.nodes.iter().filter(|n| self.has_node(n.id)).map(|n| n.id).collect();
        result.connected_exec = plan.insert.exec_connections.into_iter().filter(|c| self.connections.contains_exec(c)).collect();
        result.connected_data = plan.insert.data_connections.into_iter().filter(|c| self.connections.contains_data(c)).collect();
        result
    }
}

#[cfg(test)]
mod tests;
