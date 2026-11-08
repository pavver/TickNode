use super::*;

impl Actor {
    /// Applies an editor command; returns events to broadcast to all editors and the IDs of nodes
    /// created for the initiator (`ServerMessage::Created`).
    pub(super) fn apply(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<(Vec<ServerMessage>, Option<Vec<NodeId>>), UiMessage> {
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. } | ClientMessage::SetPluginEnabled { .. } | ClientMessage::ConfigurePlugin { .. });
        let mut created = None;
        let events = match msg {
            ClientMessage::AddNode { type_name, position } => {
                let id = NodeId::generate();
                self.engine.create_node(id, &type_name, &[]).map_err(|e| e.ui_message())?;
                self.positions.insert(id, position);
                created = Some(vec![id]);
                vec![ServerMessage::NodeAdded(self.instance(id).expect("the node was just created"))]
            }
            ClientMessage::RemoveNode(id) => {
                self.require_node(id)?;
                self.engine.remove_node(id);
                self.positions.remove(&id);
                self.traces.remove(&id);
                vec![ServerMessage::NodeRemoved(id)]
            }
            ClientMessage::MoveNode { id, position } => {
                self.require_node(id)?;
                self.positions.insert(id, position);
                vec![ServerMessage::NodeMoved { id, position }]
            }
            ClientMessage::SetProperty { id, name, value } => {
                let removed = self.engine.set_node_property(id, &name, value).map_err(|e| e.ui_message())?;
                let mut events = removed_events(removed);
                events.push(ServerMessage::NodeUpdated(self.instance(id).expect("the node exists")));
                events
            }
            ClientMessage::SetPinSplit { node, pin, split } => {
                let removed = self.engine.set_pin_split(node, pin, split).map_err(|e| e.ui_message())?;
                let mut events = removed_events(removed);
                events.push(ServerMessage::NodeUpdated(self.instance(node).expect("the node exists")));
                events
            }
            ClientMessage::SetInputValue { id, pin, value } => {
                self.engine.set_input_value(id, pin, value).map_err(|e| e.ui_message())?;
                vec![ServerMessage::NodeUpdated(self.instance(id).expect("the node exists"))]
            }
            ClientMessage::ConnectExec(c) => {
                self.engine.connect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
                vec![ServerMessage::ExecConnected(c)]
            }
            ClientMessage::DisconnectExec(c) => {
                if self.engine.disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin) {
                    vec![ServerMessage::ExecDisconnected(c)]
                } else {
                    vec![]
                }
            }
            ClientMessage::ConnectData(c) => {
                self.engine.connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
                vec![ServerMessage::DataConnected(c)]
            }
            ClientMessage::DisconnectData { dst_node, dst_pin } => {
                if self.engine.disconnect_data(dst_node, dst_pin) {
                    vec![ServerMessage::DataDisconnected { dst_node, dst_pin }]
                } else {
                    vec![]
                }
            }
            ClientMessage::TriggerNode { id, pin } => {
                self.require_node(id)?;
                self.engine.trigger_node(id, pin);
                vec![]
            }
            ClientMessage::Paste(snippet) => {
                if snippet.nodes.len() > MAX_PASTE_NODES {
                    return Err(UiMessage::new(
                        "error-paste-too-many",
                        format!("Too many nodes to paste: {} (maximum {})", snippet.nodes.len(), MAX_PASTE_NODES),
                    ).arg("count", snippet.nodes.len()).arg("max", MAX_PASTE_NODES));
                }
                let (imported, warnings) = self.engine.import_snippet(&snippet);
                if !warnings.is_empty() {
                    let details = warnings.join("; ");
                    let _ = reply.send(ServerMessage::Error(
                        UiMessage::new("error-paste-incomplete", format!("Not everything was pasted: {details}")).arg("details", details),
                    ));
                }
                let positions: HashMap<NodeId, Position> = snippet.nodes.iter().map(|n| (n.id, n.position)).collect();
                let ids: Vec<NodeId> = imported.iter().map(|(_, new)| *new).collect();
                for (old, new) in &imported {
                    self.positions.insert(*new, positions.get(old).copied().unwrap_or_default());
                }
                let mut events: Vec<ServerMessage> = ids.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeAdded).collect();
                let inner = self.engine.export_snippet(&ids);
                events.extend(inner.exec_connections.into_iter().map(ServerMessage::ExecConnected));
                events.extend(inner.data_connections.into_iter().map(ServerMessage::DataConnected));
                created = Some(ids);
                events
            }
            ClientMessage::SetPluginEnabled { name, enabled } => {
                let outcome = self.plugins.set_enabled(&mut self.engine, &name, enabled)?;
                if let Some(warning) = outcome.warning {
                    let _ = reply.send(ServerMessage::Error(warning));
                }
                // Send new definitions first so the editor knows the pins of converted nodes
                let mut events = vec![self.catalog()];
                events.extend(outcome.change.replaced.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeUpdated));
                if !outcome.change.removed.exec.is_empty() || !outcome.change.removed.data.is_empty() {
                    self.dirty_since.get_or_insert_with(Instant::now);
                }
                events.extend(removed_events(outcome.change.removed));
                events.extend(self.plugins.info(&self.engine, &name).map(ServerMessage::PluginUpdated));
                events
            }
            ClientMessage::ConfigurePlugin { name, values } => {
                self.plugins.configure(&mut self.engine, &name, values)?;
                self.plugins.info(&self.engine, &name).map(ServerMessage::PluginUpdated).into_iter().collect()
            }
        };
        if mutates && !events.is_empty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok((events, created))
    }
}

fn removed_events(removed: RemovedConnections) -> Vec<ServerMessage> {
    let exec = removed.exec.into_iter().map(ServerMessage::ExecDisconnected);
    let data = removed.data.into_iter().map(|c| ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin });
    exec.chain(data).collect()
}
