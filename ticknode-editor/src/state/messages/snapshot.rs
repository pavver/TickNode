//! Reconcile a reconnect snapshot while keeping surviving node signals alive.

use super::*;

impl Editor {
    pub(super) fn remove_absent_nodes(&self, nodes: &[NodeInstance]) -> Vec<ClientNode> {
        // Remove disappeared nodes; keep open inspector and inline fields alive across reconnects.
        let fresh: BTreeSet<NodeId> = nodes.iter().map(|i| i.model.id).collect();
        self.nodes
            .try_update(|map| {
                let ids: Vec<NodeId> = map
                    .keys()
                    .filter(|id| !fresh.contains(id))
                    .copied()
                    .collect();
                ids.iter().filter_map(|id| map.remove(id)).collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn replace_graph(
        &self,
        nodes: Vec<NodeInstance>,
        exec_connections: Vec<ConnectionModel>,
        data_connections: Vec<ConnectionModel>,
        traces: Vec<(NodeTrace, u64)>,
        gone: Vec<ClientNode>,
    ) {
        let count = nodes.len();
        for instance in nodes {
            self.upsert_node(instance);
        }
        dispose_later(gone);
        self.exec.set(exec_connections);
        self.data.set(data_connections);
        let now = now_ms();
        for (trace, ago_ms) in traces {
            if let Some(node) = self.node(trace.node_id) {
                node.set_trace(trace, now - ago_ms as f64);
            }
        }
        self.prune_selection();
        self.log(
            LogKind::System,
            format!(
                "[UI] Graph received: {count} {}",
                if count == 1 { "node" } else { "nodes" }
            ),
        );
        if !self.fitted.get_value() {
            self.fitted.set_value(true);
            let ed = *self;
            // Wait until nodes are mounted and the canvas has dimensions.
            request_animation_frame(move || crate::canvas::fit_view(ed));
        }
    }
}
