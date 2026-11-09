use super::*;
use ticknode_shared::{GraphModel, HistoryState};

const MAX_HISTORY: usize = 100;

#[derive(Default)]
pub(super) struct History {
    undo: Vec<GraphModel>,
    redo: Vec<GraphModel>,
}

impl History {
    pub(super) fn state(&self) -> HistoryState {
        HistoryState { can_undo: !self.undo.is_empty(), can_redo: !self.redo.is_empty() }
    }

    fn record(&mut self, before: GraphModel) {
        Self::push(&mut self.undo, before);
        self.redo.clear();
    }

    fn push(stack: &mut Vec<GraphModel>, graph: GraphModel) {
        if stack.len() == MAX_HISTORY { stack.remove(0); }
        stack.push(graph);
    }
}

fn undoable(msg: &ClientMessage) -> bool {
    !matches!(msg, ClientMessage::TriggerNode { .. } | ClientMessage::SetPluginEnabled { .. }
        | ClientMessage::ConfigurePlugin { .. } | ClientMessage::Undo | ClientMessage::Redo)
}

pub(super) fn batch_error() -> UiMessage {
    UiMessage::new("error-batch-command", "This command is not allowed inside a batch")
}

impl Actor {
    pub(super) fn graph_snapshot(&self) -> GraphModel {
        let mut graph = self.engine.export_graph();
        for node in &mut graph.nodes {
            node.position = self.positions.get(&node.id).copied().unwrap_or_default();
        }
        graph.wire_routes = super::routes::sorted(&self.routes);
        graph
    }

    /// Records one snapshot for a successful edit or a partially successful batch.
    pub(super) fn apply(
        &mut self, msg: ClientMessage, reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(Vec<ServerMessage>, Option<Vec<NodeId>>), UiMessage> {
        let previous = self.history.state();
        let before = undoable(&msg).then(|| self.graph_snapshot());
        let (mut events, created) = match msg {
            ClientMessage::Undo => (self.restore_history(false), None),
            ClientMessage::Redo => (self.restore_history(true), None),
            ClientMessage::Batch(commands) => self.apply_batch(commands, reply),
            msg => self.apply_command(msg, reply)?,
        };
        self.prune_routes();
        if !events.is_empty() && let Some(before) = before {
            self.history.record(before);
        }
        let state = self.history.state();
        if state != previous { events.push(ServerMessage::History(state)); }
        Ok((events, created))
    }

    fn apply_batch(
        &mut self, commands: Vec<ClientMessage>, reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> (Vec<ServerMessage>, Option<Vec<NodeId>>) {
        let mut events = Vec::new();
        let mut created = None;
        let mut first_error = None;
        // Capture per-command warnings too, so a batch reports only its first error.
        let (local_reply, mut errors) = mpsc::unbounded_channel();
        for msg in commands {
            let result = if !undoable(&msg) || matches!(msg, ClientMessage::Batch(_)) {
                Err(batch_error())
            } else {
                self.apply_command(msg, &local_reply)
            };
            match result {
                Ok((changes, ids)) => {
                    events.extend(changes);
                    if let Some(ids) = ids { created.get_or_insert_with(Vec::new).extend(ids); }
                }
                Err(error) => { first_error.get_or_insert(error); }
            }
            self.prune_routes();
            while let Ok(ServerMessage::Error(error)) = errors.try_recv() {
                first_error.get_or_insert(error);
            }
        }
        if let Some(error) = first_error { let _ = reply.send(ServerMessage::Error(error)); }
        (events, created)
    }

    fn restore_history(&mut self, redo: bool) -> Vec<ServerMessage> {
        let target = if redo { self.history.redo.pop() } else { self.history.undo.pop() };
        let Some(target) = target else { return vec![] };
        let current = self.graph_snapshot();
        History::push(if redo { &mut self.history.undo } else { &mut self.history.redo }, current);
        self.reconcile_graph(&target)
    }

    /// Reconciles a snapshot and reports the same graph changes for history and settings paste.
    pub(super) fn reconcile_graph(&mut self, target: &GraphModel) -> Vec<ServerMessage> {
        let exec_before = self.engine.exec_connections().to_vec();
        let data_before = self.engine.data_connections().to_vec();
        let result = self.engine.reconcile(target);
        for warning in result.warnings { tracing::warn!("Graph reconcile: {warning}"); }

        let mut events = Vec::new();
        for id in &result.removed_nodes {
            self.traces.remove(id);
            if !self.engine.has_node(*id) {
                self.positions.remove(id);
                events.push(ServerMessage::NodeRemoved(*id));
            }
        }
        let exists = |c: &ticknode_shared::ConnectionModel| self.engine.has_node(c.src_node) && self.engine.has_node(c.dst_node);
        let disconnected = RemovedConnections {
            exec: result.disconnected.exec.into_iter().filter(|c| exists(c) && !self.engine.exec_connections().contains(c)).collect(),
            data: result.disconnected.data.into_iter().filter(|c| exists(c) && !self.engine.data_connections().contains(c)).collect(),
        };
        events.extend(super::commands::removed_events(disconnected));
        let moved = self.restore_positions(target, &result.added_nodes);
        // A recreated node keeps its editor identity and selection, as do unchanged wires.
        events.extend(result.added_nodes.into_iter().filter_map(|id| self.instance(id)).map(|instance| {
            if result.removed_nodes.contains(&instance.model.id) { ServerMessage::NodeUpdated(instance) }
            else { ServerMessage::NodeAdded(instance) }
        }));
        events.extend(moved);
        events.extend(result.connected_exec.into_iter().filter(|c| !exec_before.contains(c)).map(ServerMessage::ExecConnected));
        events.extend(result.connected_data.into_iter().filter(|c| !data_before.contains(c)).map(ServerMessage::DataConnected));
        events.extend(self.restore_routes(&target.wire_routes));
        self.dirty_since.get_or_insert_with(Instant::now);
        events
    }
    fn restore_positions(&mut self, target: &GraphModel, added_nodes: &[NodeId]) -> Vec<ServerMessage> {
        let mut moved = Vec::new();
        for node in &target.nodes {
            if !self.engine.has_node(node.id) { continue; }
            let old = self.positions.insert(node.id, node.position).unwrap_or_default();
            if !added_nodes.contains(&node.id) && old != node.position {
                moved.push(ServerMessage::NodeMoved { id: node.id, position: node.position });
            }
        }
        moved
    }
}
