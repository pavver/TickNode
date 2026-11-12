use super::*;
use ticknode_shared::{GraphModel, HistoryState, SheetId};

const MAX_HISTORY: usize = 100;

#[derive(Default)]
pub(crate) struct History {
    pub(super) undo: Vec<GraphModel>,
    pub(super) redo: Vec<GraphModel>,
}

impl History {
    pub(super) fn state(&self) -> HistoryState {
        HistoryState {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    pub(super) fn record(&mut self, before: GraphModel) {
        Self::push(&mut self.undo, before);
        self.redo.clear();
    }

    pub(super) fn push(stack: &mut Vec<GraphModel>, graph: GraphModel) {
        if stack.len() == MAX_HISTORY {
            stack.remove(0);
        }
        stack.push(graph);
    }
}

pub(super) fn undoable(msg: &ClientMessage) -> bool {
    !matches!(
        msg,
        ClientMessage::TriggerNode { .. }
            | ClientMessage::SetPluginEnabled { .. }
            | ClientMessage::ConfigurePlugin { .. }
            | ClientMessage::SetServerSettings(_)
            | ClientMessage::Undo
            | ClientMessage::Redo
            | ClientMessage::OpenSheet(_)
            | ClientMessage::CreateSheet { .. }
            | ClientMessage::UpdateSheet(_)
            | ClientMessage::DeleteSheet(_)
            | ClientMessage::MoveSheet { .. }
            | ClientMessage::DuplicateSheet(_)
            | ClientMessage::CreateVariable(_)
            | ClientMessage::SetVariable { .. }
            | ClientMessage::DeleteVariable(_)
    )
}

pub(super) fn batch_error() -> UiMessage {
    UiMessage::new(
        "error-batch-command",
        "This command is not allowed inside a batch",
    )
}

impl Actor {
    pub(super) fn graph_snapshot(&self) -> GraphModel {
        let mut graph = self.engine.export_graph();
        for node in &mut graph.nodes {
            node.position = self.positions.get(&node.id).copied().unwrap_or_default();
            node.size = self.sizes.get(&node.id).copied();
        }
        graph.wire_routes = super::routes::sorted(&self.routes);
        graph.frames = self.frames.values().cloned().collect();
        graph
    }

    /// Records one snapshot for a successful edit or a partially successful batch.
    pub(super) fn apply(
        &mut self,
        msg: ClientMessage,
        sheet: Option<SheetId>,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(Vec<(Option<SheetId>, ServerMessage)>, Option<Vec<NodeId>>), UiMessage> {
        let sheet = sheet.or_else(|| self.sheets.first().map(|sh| sh.id));
        let before = if undoable(&msg) {
            let s = sheet.ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
            if !self.sheets.iter().any(|sh| sh.id == s) {
                return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
            }
            Some((
                s,
                self.sheet_graph(s),
                self.histories.get(&s).map_or_default(|h| h.state()),
            ))
        } else {
            None
        };

        let (mut events, created) = match msg {
            ClientMessage::Undo => {
                let s =
                    sheet.ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
                if !self.sheets.iter().any(|sh| sh.id == s) {
                    return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                }
                (self.restore_history(s, false), None)
            }
            ClientMessage::Redo => {
                let s =
                    sheet.ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
                if !self.sheets.iter().any(|sh| sh.id == s) {
                    return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                }
                (self.restore_history(s, true), None)
            }
            ClientMessage::Batch(commands) => self.apply_batch(commands, sheet, reply),
            msg => self.apply_command(msg, sheet, reply)?,
        };

        self.prune_routes();
        let prev_state = before.as_ref().map(|(_, _, prev)| *prev);
        if !events.is_empty()
            && let Some((s, snap, _)) = before
        {
            self.histories.entry(s).or_default().record(snap);
            self.dirty_sheets.insert(s);
        }

        if let Some(s) = sheet {
            let state = self.histories.get(&s).map_or_default(|h| h.state());
            if prev_state.is_some_and(|prev| prev != state) {
                events.push((Some(s), ServerMessage::History(state)));
            }
        }

        Ok((events, created))
    }

    fn apply_batch(
        &mut self,
        commands: Vec<ClientMessage>,
        sheet: Option<SheetId>,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> (Vec<(Option<SheetId>, ServerMessage)>, Option<Vec<NodeId>>) {
        let mut events = Vec::new();
        let mut created = None;
        let mut first_error = None;
        let (local_reply, mut errors) = mpsc::unbounded_channel();
        for msg in commands {
            let result = if !undoable(&msg) || matches!(msg, ClientMessage::Batch(_)) {
                Err(batch_error())
            } else {
                self.apply_command(msg, sheet, &local_reply)
            };
            match result {
                Ok((changes, ids)) => {
                    events.extend(changes);
                    if let Some(ids) = ids {
                        created.get_or_insert_with(Vec::new).extend(ids);
                    }
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
            self.prune_routes();
            while let Ok(ServerMessage::Error(error)) = errors.try_recv() {
                first_error.get_or_insert(error);
            }
        }
        if let Some(error) = first_error {
            let _ = reply.send(ServerMessage::Error(error));
        }
        (events, created)
    }

    fn restore_history(&mut self, s: SheetId, redo: bool) -> Vec<(Option<SheetId>, ServerMessage)> {
        let target_sheet = {
            let history = self.histories.entry(s).or_default();
            if redo {
                history.redo.pop()
            } else {
                history.undo.pop()
            }
        };
        let Some(target_sheet) = target_sheet else {
            return vec![];
        };
        let current_sheet = self.sheet_graph(s);
        {
            let history = self.histories.entry(s).or_default();
            History::push(
                if redo {
                    &mut history.undo
                } else {
                    &mut history.redo
                },
                current_sheet,
            );
        }

        let mut full_target = self.graph_snapshot();
        full_target
            .nodes
            .retain(|n| self.node_sheet.get(&n.id) != Some(&s));
        full_target
            .exec_connections
            .retain(|c| self.node_sheet.get(&c.src_node) != Some(&s));
        full_target
            .data_connections
            .retain(|c| self.node_sheet.get(&c.src_node) != Some(&s));
        full_target
            .wire_routes
            .retain(|r| self.node_sheet.get(&r.connection.src_node) != Some(&s));
        full_target
            .detached_connections
            .retain(|d| self.node_sheet.get(&d.connection.src_node) != Some(&s));
        full_target
            .frames
            .retain(|f| self.frame_sheet.get(&f.id) != Some(&s));

        full_target.nodes.extend(target_sheet.nodes);
        full_target
            .exec_connections
            .extend(target_sheet.exec_connections);
        full_target
            .data_connections
            .extend(target_sheet.data_connections);
        full_target.wire_routes.extend(target_sheet.wire_routes);
        full_target
            .detached_connections
            .extend(target_sheet.detached_connections);
        full_target.frames.extend(target_sheet.frames);

        let mut events = self.reconcile_graph_for_sheet(&full_target, s);
        let state = self.histories.get(&s).map_or_default(|h| h.state());
        events.push((Some(s), ServerMessage::History(state)));
        self.dirty_sheets.insert(s);
        events
    }

    pub(super) fn reconcile_graph_for_sheet(
        &mut self,
        target: &GraphModel,
        s: SheetId,
    ) -> Vec<(Option<SheetId>, ServerMessage)> {
        self.reconcile_graph_edit_for_sheet(target, None, s)
    }

    pub(super) fn reconcile_graph_edit_for_sheet(
        &mut self,
        target: &GraphModel,
        user_edit: Option<NodeId>,
        s: SheetId,
    ) -> Vec<(Option<SheetId>, ServerMessage)> {
        let detached_before = self.engine.detached_connections().to_vec();
        let exec_before = self.engine.exec_connections().to_vec();
        let data_before = self.engine.data_connections().to_vec();
        let result = match user_edit {
            Some(id) => self.engine.reconcile_settings(target, id),
            None => self.engine.reconcile(target),
        };
        for warning in result.warnings {
            tracing::warn!("Graph reconcile: {warning}");
        }

        for id in &result.added_nodes {
            self.node_sheet.insert(*id, s);
            self.engine.set_node_sheet(*id, Some(s));
        }
        let sheet_enabled = self
            .sheets
            .iter()
            .find(|sh| sh.id == s)
            .map_or(true, |sh| sh.enabled);
        if !sheet_enabled {
            self.engine
                .set_disabled(result.added_nodes.iter().copied(), true);
        }

        let detached_after = self.engine.detached_connections().to_vec();
        let mut events: Vec<_> = detached_before
            .iter()
            .filter(|d| !detached_after.contains(d))
            .copied()
            .map(ServerMessage::DetachedRemoved)
            .collect();
        let detached_added: Vec<_> = detached_after
            .iter()
            .filter(|d| !detached_before.contains(d))
            .copied()
            .collect();
        for id in &result.removed_nodes {
            self.traces.remove(id);
            if !self.engine.has_node(*id) {
                self.node_sheet.remove(id);
                self.positions.remove(id);
                self.sizes.remove(id);
                events.push(ServerMessage::NodeRemoved(*id));
            }
        }
        let exists = |c: &ticknode_shared::ConnectionModel| {
            self.engine.has_node(c.src_node) && self.engine.has_node(c.dst_node)
        };
        let disconnected = RemovedConnections {
            exec: result
                .disconnected
                .exec
                .into_iter()
                .filter(|c| {
                    exists(c)
                        && !self.engine.exec_connections().contains(c)
                        && !detached_after.iter().any(|d| d.exec && d.connection == *c)
                })
                .collect(),
            data: result
                .disconnected
                .data
                .into_iter()
                .filter(|c| {
                    exists(c)
                        && !self.engine.data_connections().contains(c)
                        && !detached_after.iter().any(|d| !d.exec && d.connection == *c)
                })
                .collect(),
        };
        events.extend(super::commands::removed_events(disconnected));
        let moved = self.restore_positions(target, &result.added_nodes);
        let resized = self.restore_sizes(target, &result.added_nodes);
        events.extend(
            result
                .added_nodes
                .iter()
                .copied()
                .filter_map(|id| self.instance(id))
                .map(|instance| {
                    if result.removed_nodes.contains(&instance.model.id) {
                        ServerMessage::NodeUpdated(instance)
                    } else {
                        ServerMessage::NodeAdded(instance)
                    }
                }),
        );
        events.extend(
            result
                .updated_nodes
                .into_iter()
                .filter(|id| !result.added_nodes.contains(id))
                .filter_map(|id| self.instance(id))
                .map(ServerMessage::NodeUpdated),
        );
        events.extend(moved);
        events.extend(resized);
        events.extend(
            result
                .connected_exec
                .into_iter()
                .filter(|c| !exec_before.contains(c))
                .map(ServerMessage::ExecConnected),
        );
        events.extend(
            result
                .connected_data
                .into_iter()
                .filter(|c| !data_before.contains(c))
                .map(ServerMessage::DataConnected),
        );
        events.extend(detached_added.into_iter().map(ServerMessage::Detached));
        events.extend(self.restore_routes_for_sheet(s, &target.wire_routes));
        events.extend(self.restore_frames_for_sheet(s, &target.frames));
        self.dirty_since.get_or_insert_with(Instant::now);
        events.into_iter().map(|e| (Some(s), e)).collect()
    }

    fn restore_positions(
        &mut self,
        target: &GraphModel,
        added_nodes: &[NodeId],
    ) -> Vec<ServerMessage> {
        let mut moved = Vec::new();
        for node in &target.nodes {
            if !self.engine.has_node(node.id) {
                continue;
            }
            let old = self
                .positions
                .insert(node.id, node.position)
                .unwrap_or_default();
            if !added_nodes.contains(&node.id) && old != node.position {
                moved.push(ServerMessage::NodeMoved {
                    id: node.id,
                    position: node.position,
                });
            }
        }
        moved
    }
}
