use super::*;
use ticknode_shared::{ConnectionEnd, ConnectionModel, DataValue, PinDirection, PinType, PinId};

type Applied = (Vec<ServerMessage>, Option<Vec<NodeId>>);

impl Actor {
    /// Applies an editor command; returns events to broadcast to all editors and the IDs of nodes
    /// created for the initiator (`ServerMessage::Created`).
    pub(super) fn apply_command(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Applied, UiMessage> {
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. } | ClientMessage::SetPluginEnabled { .. } | ClientMessage::ConfigurePlugin { .. });
        let only = |events: Vec<ServerMessage>| (events, None);
        let result = match msg {
            ClientMessage::AddNode { type_name, position } => self.add_node(&type_name, position)?,
            ClientMessage::AddNodeConnected { type_name, position, from, pin } => self.add_node_connected(&type_name, position, from, pin)?,
            ClientMessage::ConfirmPins(id) => only(if self.engine.confirm_pins(id).map_err(|e| e.ui_message())? {
                vec![ServerMessage::NodeUpdated(self.instance(id).expect("the node exists"))]
            } else { vec![] }),
            ClientMessage::RemoveDetached(d) => only(if self.engine.remove_detached(&d) { vec![ServerMessage::DetachedRemoved(d)] } else { vec![] }),
            ClientMessage::RemoveNode(id) => only(self.remove_node(id)?),
            ClientMessage::SetWireRoute(route) => only(self.set_wire_route(route)?),
            ClientMessage::MoveNode { id, position } => only(self.move_node(id, position)?),
            ClientMessage::Paste(snippet) => self.paste(snippet, reply)?,
            ClientMessage::PasteSettings { id, type_name, properties } => only(self.paste_settings(id, &type_name, properties)?),
            ClientMessage::SetProperty { id, name, value } => only(self.set_property(id, &name, value)?),
            ClientMessage::SetPinSplit { node, pin, split } => only(self.set_pin_split(node, pin, split)?),
            ClientMessage::SetInputValue { id, pin, value } => only(self.set_input_value(id, pin, value)?),
            ClientMessage::ConnectExec(c) => only(self.connect_exec(c)?),
            ClientMessage::DisconnectExec(c) => only(self.disconnect_exec(c)),
            ClientMessage::ConnectData(c) => only(self.connect_data(c)?),
            ClientMessage::DisconnectData { dst_node, dst_pin } => only(self.disconnect_data(dst_node, dst_pin)),
            ClientMessage::TriggerNode { id, pin } => only(self.trigger(id, pin)?),
            ClientMessage::SetPluginEnabled { name, enabled } => only(self.set_plugin_enabled(&name, enabled, reply)?),
            ClientMessage::ConfigurePlugin { name, values } => only(self.configure_plugin(&name, values)?),
            ClientMessage::Undo | ClientMessage::Redo | ClientMessage::Batch(_) => return Err(super::history::batch_error()),
        };
        self.mark_command_dirty(mutates, &result.0);
        Ok(result)
    }

    fn mark_command_dirty(&mut self, mutates: bool, events: &[ServerMessage]) {
        if mutates && !events.is_empty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
    }

    fn add_node(&mut self, type_name: &str, position: Position) -> Result<Applied, UiMessage> {
        let id = NodeId::generate();
        self.engine.create_node(id, type_name, &[]).map_err(|e| e.ui_message())?;
        self.positions.insert(id, position);
        Ok((vec![ServerMessage::NodeAdded(self.instance(id).expect("the node was just created"))], Some(vec![id])))
    }

    fn add_node_connected(&mut self, type_name: &str, position: Position, from: ConnectionEnd, pin: PinId) -> Result<Applied, UiMessage> {
        self.require_node(from.node)?;
        let definition = self.engine.node_definition(from.node).expect("the node exists");
        let origin = definition.pins.iter().find(|p| p.id == from.pin)
            .ok_or_else(|| ticknode_core::EngineError::InvalidPinConnection.ui_message())?;
        let (mut events, created) = self.add_node(type_name, position)?;
        let id = created.as_ref().expect("add_node returns an ID")[0];
        // A pin ID may occur in both directions; the chosen new pin disambiguates it.
        let new_definition = self.engine.node_definition(id).expect("the node was just created");
        let origin = definition.pins.iter().find(|p| p.id == from.pin && new_definition.pins.iter().any(|n|
            n.id == pin && n.direction != p.direction && matches!((&p.pin_type, &n.pin_type),
                (PinType::Exec, PinType::Exec) | (PinType::Data(_), PinType::Data(_)))))
            .unwrap_or(origin);
        let c = if origin.direction == PinDirection::Output {
            ConnectionModel { src_node: from.node, src_pin: from.pin, dst_node: id, dst_pin: pin }
        } else {
            ConnectionModel { src_node: id, src_pin: pin, dst_node: from.node, dst_pin: from.pin }
        };
        let connected = match origin.pin_type {
            PinType::Exec => self.connect_exec(c),
            PinType::Data(_) => self.connect_data(c),
        };
        match connected {
            Ok(changes) => { events.extend(changes); Ok((events, created)) }
            Err(error) => {
                self.engine.remove_node(id);
                self.positions.remove(&id);
                Err(error)
            }
        }
    }

    fn remove_node(&mut self, id: NodeId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_node(id)?;
        let detached: Vec<_> = self.engine.detached_connections().iter()
            .filter(|d| d.connection.src_node == id || d.connection.dst_node == id).copied().collect();
        self.engine.remove_node(id);
        self.positions.remove(&id);
        self.traces.remove(&id);
        let mut events: Vec<_> = detached.into_iter().map(ServerMessage::DetachedRemoved).collect();
        events.push(ServerMessage::NodeRemoved(id));
        Ok(events)
    }

    fn move_node(&mut self, id: NodeId, position: Position) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_node(id)?;
        if self.positions.get(&id).copied().unwrap_or_default() == position { return Ok(vec![]); }
        self.positions.insert(id, position);
        Ok(vec![ServerMessage::NodeMoved { id, position }])
    }

    fn paste(&mut self, snippet: ticknode_shared::GraphModel, reply: &mpsc::UnboundedSender<ServerMessage>) -> Result<Applied, UiMessage> {
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
        events.extend(inner.detached_connections.into_iter().map(ServerMessage::Detached));
        events.extend(self.paste_routes(&snippet.wire_routes, &imported));
        Ok((events, Some(ids)))
    }

    fn set_property(&mut self, id: NodeId, name: &str, value: DataValue) -> Result<Vec<ServerMessage>, UiMessage> {
        let before = self.engine.node_model(id);
        let removed = self.engine.set_node_property(id, name, value).map_err(|e| e.ui_message())?;
        Ok(self.updated_events(id, before, removed))
    }

    fn set_pin_split(&mut self, node: NodeId, pin: PinId, split: bool) -> Result<Vec<ServerMessage>, UiMessage> {
        let before = self.engine.node_model(node);
        let removed = self.engine.set_pin_split(node, pin, split).map_err(|e| e.ui_message())?;
        Ok(self.updated_events(node, before, removed))
    }

    fn set_input_value(&mut self, id: NodeId, pin: PinId, value: Option<DataValue>) -> Result<Vec<ServerMessage>, UiMessage> {
        let before = self.engine.node_model(id);
        self.engine.set_input_value(id, pin, value).map_err(|e| e.ui_message())?;
        Ok(self.updated_events(id, before, RemovedConnections::default()))
    }

    fn updated_events(&self, id: NodeId, before: Option<ticknode_shared::NodeModel>, removed: RemovedConnections) -> Vec<ServerMessage> {
        if self.engine.node_model(id) == before { return vec![]; }
        let mut events = removed_events(removed);
        events.push(ServerMessage::NodeUpdated(self.instance(id).expect("the node exists")));
        events
    }

    fn disconnect_exec(&mut self, c: ConnectionModel) -> Vec<ServerMessage> {
        if self.engine.disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin) { vec![ServerMessage::ExecDisconnected(c)] } else { vec![] }
    }

    fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId) -> Vec<ServerMessage> {
        if self.engine.disconnect_data(dst_node, dst_pin) { vec![ServerMessage::DataDisconnected { dst_node, dst_pin }] } else { vec![] }
    }

    fn connect_exec(&mut self, c: ConnectionModel) -> Result<Vec<ServerMessage>, UiMessage> {
        if self.engine.exec_connections().contains(&c) { return Ok(vec![]); }
        self.engine.connect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
        Ok(vec![ServerMessage::ExecConnected(c)])
    }

    fn connect_data(&mut self, c: ConnectionModel) -> Result<Vec<ServerMessage>, UiMessage> {
        if self.engine.data_connections().contains(&c) { return Ok(vec![]); }
        let replaced = self.engine.data_connections().iter().any(|old| old.dst_node == c.dst_node && old.dst_pin == c.dst_pin);
        self.engine.connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
        let mut events = Vec::new();
        if replaced { events.push(ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin }); }
        events.push(ServerMessage::DataConnected(c));
        Ok(events)
    }

    fn trigger(&mut self, id: NodeId, pin: PinId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_node(id)?;
        self.engine.trigger_node(id, pin);
        Ok(vec![])
    }

    fn configure_plugin(&mut self, name: &str, values: Vec<(String, ticknode_shared::SettingUpdate)>) -> Result<Vec<ServerMessage>, UiMessage> {
        self.plugins.configure(&mut self.engine, name, values)?;
        Ok(self.plugins.info(&self.engine, name).map(ServerMessage::PluginUpdated).into_iter().collect())
    }

    fn set_plugin_enabled(&mut self, name: &str, enabled: bool, reply: &mpsc::UnboundedSender<ServerMessage>) -> Result<Vec<ServerMessage>, UiMessage> {
        let outcome = self.plugins.set_enabled(&mut self.engine, name, enabled)?;
        self.history = Default::default();
        if let Some(warning) = outcome.warning {
            let _ = reply.send(ServerMessage::Error(warning));
        }
        // Send new definitions first so the editor knows the pins of converted nodes
        let mut events = vec![self.catalog()];
        events.extend(outcome.change.replaced.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeUpdated));
        if !outcome.change.replaced.is_empty() || !outcome.change.detached.is_empty() || !outcome.change.removed.exec.is_empty() || !outcome.change.removed.data.is_empty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        events.extend(removed_events(outcome.change.removed));
        events.extend(outcome.change.detached.into_iter().map(ServerMessage::Detached));
        events.extend(self.plugins.info(&self.engine, name).map(ServerMessage::PluginUpdated));
        Ok(events)
    }
}

pub(super) fn removed_events(removed: RemovedConnections) -> Vec<ServerMessage> {
    let exec = removed.exec.into_iter().map(ServerMessage::ExecDisconnected);
    let data = removed.data.into_iter().map(|c| ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin });
    exec.chain(data).collect()
}
