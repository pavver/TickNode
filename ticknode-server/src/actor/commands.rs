use super::*;
use ticknode_shared::{ConnectionEnd, ConnectionModel, DataValue, PinDirection, PinType, PinId, SheetId};

type Applied = (Vec<(Option<SheetId>, ServerMessage)>, Option<Vec<NodeId>>);

impl Actor {
    pub(super) fn require_session_node(&self, id: NodeId, sheet: SheetId) -> Result<(), UiMessage> {
        if self.engine.has_node(id) && self.node_sheet.get(&id) == Some(&sheet) {
            Ok(())
        } else {
            Err(EngineError::NodeNotFound(id).ui_message())
        }
    }

    pub(super) fn require_session_connection(&self, c: &ConnectionModel, sheet: SheetId) -> Result<(), UiMessage> {
        let src_sheet = self.node_sheet.get(&c.src_node).copied();
        let dst_sheet = self.node_sheet.get(&c.dst_node).copied();
        if src_sheet.is_none() {
            return Err(EngineError::NodeNotFound(c.src_node).ui_message());
        }
        if dst_sheet.is_none() {
            return Err(EngineError::NodeNotFound(c.dst_node).ui_message());
        }
        if src_sheet != dst_sheet {
            return Err(UiMessage::new("error-cross-sheet", "Nodes on different sheets cannot be connected"));
        }
        if src_sheet != Some(sheet) {
            return Err(EngineError::NodeNotFound(c.src_node).ui_message());
        }
        Ok(())
    }

    pub(super) fn sheet_is_enabled(&self, sheet: SheetId) -> bool {
        self.sheets.iter().find(|s| s.id == sheet).map_or(true, |s| s.enabled)
    }

    /// Applies an editor command; returns events to broadcast and the IDs of nodes created.
    pub(super) fn apply_command(
        &mut self,
        msg: ClientMessage,
        sheet: Option<SheetId>,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Applied, UiMessage> {

        // Global plugin commands
        match msg {
            ClientMessage::SetPluginEnabled { name, enabled } => {
                let events = self.set_plugin_enabled(&name, enabled, reply)?;
                return Ok((events, None));
            }
            ClientMessage::ConfigurePlugin { name, values } => {
                let events = self.configure_plugin(&name, values)?;
                return Ok((events, None));
            }
            // Sheet management commands
            ClientMessage::CreateSheet { title, caption } => {
                let _id = self.create_sheet(title, caption, reply)?;
                return Ok((vec![], None));
            }
            ClientMessage::UpdateSheet(info) => {
                self.update_sheet(info)?;
                return Ok((vec![], None));
            }
            ClientMessage::DeleteSheet(id) => {
                self.delete_sheet(id)?;
                return Ok((vec![], None));
            }
            ClientMessage::MoveSheet { id, index } => {
                self.move_sheet(id, index)?;
                return Ok((vec![], None));
            }
            ClientMessage::DuplicateSheet(id) => {
                let _new_id = self.duplicate_sheet(id, reply)?;
                return Ok((vec![], None));
            }
            ClientMessage::OpenSheet(target) => {
                if let Some(id) = target {
                    if !self.sheets.iter().any(|s| s.id == id) {
                        return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                    }
                }
                let _ = reply.send(self.snapshot(target));
                return Ok((vec![], None));
            }
            ClientMessage::Undo | ClientMessage::Redo | ClientMessage::Batch(_) => return Err(super::history::batch_error()),
            _ => {}
        }

        // All remaining commands require an open sheet
        let s = sheet.or_else(|| self.sheets.first().map(|sh| sh.id)).ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
        if !self.sheets.iter().any(|sh| sh.id == s) {
            return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
        }

        let only = |events: Vec<ServerMessage>| (events.into_iter().map(|e| (Some(s), e)).collect(), None);
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. });

        let result: Applied = match msg {
            ClientMessage::AddNode { type_name, position } => self.add_node(&type_name, position, s)?,
            ClientMessage::AddNodeWith { type_name, position, properties } => self.add_node_with(&type_name, position, properties, s)?,
            ClientMessage::AddNodeConnected { type_name, position, from, pin } => self.add_node_connected(&type_name, position, from, pin, s)?,
            ClientMessage::ConfirmPins(id) => {
                self.require_session_node(id, s)?;
                only(if self.engine.confirm_pins(id).map_err(|e| e.ui_message())? {
                    vec![ServerMessage::NodeUpdated(self.instance(id).expect("the node exists"))]
                } else { vec![] })
            }
            ClientMessage::RemoveDetached(d) => {
                self.require_session_node(d.connection.src_node, s)?;
                only(if self.engine.remove_detached(&d) { vec![ServerMessage::DetachedRemoved(d)] } else { vec![] })
            }
            ClientMessage::RemoveNode(id) => only(self.remove_node(id, s)?),
            ClientMessage::SetWireRoute(route) => only(self.set_wire_route(route, s)?),
            ClientMessage::MoveNode { id, position } => only(self.move_node(id, position, s)?),
            ClientMessage::ResizeNode { id, size } => only(self.resize_node(id, size, s)?),
            ClientMessage::SetFrame(frame) => only(self.set_frame(frame, s)?),
            ClientMessage::RemoveFrame(id) => only(self.remove_frame(id, s)),
            ClientMessage::Paste(snippet) => self.paste(snippet, s, reply)?,
            ClientMessage::PasteSettings { id, type_name, properties } => self.paste_settings_on_sheet(id, &type_name, properties, s)?,
            ClientMessage::SetProperty { id, name, value } => only(self.set_property(id, &name, value, s)?),
            ClientMessage::SetPinSplit { node, pin, split } => only(self.set_pin_split(node, pin, split, s)?),
            ClientMessage::SetInputValue { id, pin, value } => only(self.set_input_value(id, pin, value, s)?),
            ClientMessage::ConnectExec(c) => only(self.connect_exec(c, s)?),
            ClientMessage::DisconnectExec(c) => only(self.disconnect_exec(c, s)?),
            ClientMessage::ConnectData(c) => only(self.connect_data(c, s)?),
            ClientMessage::DisconnectData { dst_node, dst_pin } => only(self.disconnect_data(dst_node, dst_pin, s)?),
            ClientMessage::TriggerNode { id, pin } => only(self.trigger(id, pin, s)?),
            _ => unreachable!(),
        };

        if mutates && !result.0.is_empty() {
            self.dirty_sheets.insert(s);
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok(result)
    }

    fn add_node(&mut self, type_name: &str, position: Position, sheet: SheetId) -> Result<Applied, UiMessage> {
        let id = NodeId::generate();
        self.engine.create_node(id, type_name, &[]).map_err(|e| e.ui_message())?;
        self.node_sheet.insert(id, sheet);
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(std::iter::once(id), true);
        }
        self.positions.insert(id, position);
        Ok((vec![(Some(sheet), ServerMessage::NodeAdded(self.instance(id).expect("the node was just created")))], Some(vec![id])))
    }

    fn add_node_with(&mut self, type_name: &str, position: Position, properties: Vec<(String, DataValue)>, sheet: SheetId) -> Result<Applied, UiMessage> {
        for (k, _) in &properties {
            if k.starts_with("in:") || k.starts_with("split:") {
                return Err(UiMessage::new("error-invalid-property", format!("Invalid property: {k}")).arg("property", k));
            }
        }
        let id = NodeId::generate();
        self.engine.create_node(id, type_name, &properties).map_err(|e| e.ui_message())?;
        self.node_sheet.insert(id, sheet);
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(std::iter::once(id), true);
        }
        self.positions.insert(id, position);
        Ok((vec![(Some(sheet), ServerMessage::NodeAdded(self.instance(id).expect("the node was just created")))], Some(vec![id])))
    }

    fn add_node_connected(&mut self, type_name: &str, position: Position, from: ConnectionEnd, pin: PinId, sheet: SheetId) -> Result<Applied, UiMessage> {
        self.require_session_node(from.node, sheet)?;
        let definition = self.engine.node_definition(from.node).expect("the node exists");
        let origin = definition.pins.iter().find(|p| p.id == from.pin)
            .ok_or_else(|| ticknode_core::EngineError::InvalidPinConnection.ui_message())?;
        let (mut events, created) = self.add_node(type_name, position, sheet)?;
        let id = created.as_ref().expect("add_node returns an ID")[0];

        let new_definition = self.engine.node_definition(id).expect("the node was just created");
        let origin = definition.pins.iter().find(|p| p.id == from.pin && new_definition.pins.iter().any(|n|
            n.id == pin && n.direction != p.direction && matches!((&p.pin_type, &n.pin_type),
                (PinType::Exec, PinType::Exec) | (PinType::Data(_), PinType::Data(_)))))
            .unwrap_or(origin);
        if new_definition.adaptive && let PinType::Data(data_type) = &origin.pin_type {
            if let Err(error) = self.engine.adapt_node(id, pin, data_type) {
                self.engine.remove_node(id);
                self.node_sheet.remove(&id);
                self.positions.remove(&id);
                self.sizes.remove(&id);
                return Err(error.ui_message());
            }
            events[0] = (Some(sheet), ServerMessage::NodeAdded(self.instance(id).expect("the node exists")));
        }
        let c = if origin.direction == PinDirection::Output {
            ConnectionModel { src_node: from.node, src_pin: from.pin, dst_node: id, dst_pin: pin }
        } else {
            ConnectionModel { src_node: id, src_pin: pin, dst_node: from.node, dst_pin: from.pin }
        };
        let connected = match origin.pin_type {
            PinType::Exec => self.connect_exec(c, sheet),
            PinType::Data(_) => self.connect_data(c, sheet),
        };
        match connected {
            Ok(changes) => {
                events.extend(changes.into_iter().map(|e| (Some(sheet), e)));
                Ok((events, created))
            }
            Err(error) => {
                self.engine.remove_node(id);
                self.node_sheet.remove(&id);
                self.positions.remove(&id);
                self.sizes.remove(&id);
                Err(error)
            }
        }
    }

    fn remove_node(&mut self, id: NodeId, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        let detached: Vec<_> = self.engine.detached_connections().iter()
            .filter(|d| d.connection.src_node == id || d.connection.dst_node == id).copied().collect();
        self.engine.remove_node(id);
        self.node_sheet.remove(&id);
        self.positions.remove(&id);
        self.sizes.remove(&id);
        self.traces.remove(&id);
        let mut events: Vec<_> = detached.into_iter().map(ServerMessage::DetachedRemoved).collect();
        events.push(ServerMessage::NodeRemoved(id));
        Ok(events)
    }

    fn move_node(&mut self, id: NodeId, position: Position, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        if self.positions.get(&id).copied().unwrap_or_default() == position { return Ok(vec![]); }
        self.positions.insert(id, position);
        Ok(vec![ServerMessage::NodeMoved { id, position }])
    }

    fn paste(&mut self, snippet: ticknode_shared::GraphModel, sheet: SheetId, reply: &mpsc::UnboundedSender<ServerMessage>) -> Result<Applied, UiMessage> {
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
            self.node_sheet.insert(*new, sheet);
            self.positions.insert(*new, positions.get(old).copied().unwrap_or_default());
            if let Some(size) = snippet.nodes.iter().find(|n| n.id == *old).and_then(|n| n.size).and_then(NodeSize::validated) {
                self.sizes.insert(*new, size);
            }
        }
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(ids.iter().copied(), true);
        }
        let mut events: Vec<ServerMessage> = ids.iter().filter_map(|id| self.instance(*id)).map(ServerMessage::NodeAdded).collect();
        let inner = self.engine.export_snippet(&ids);
        events.extend(inner.exec_connections.into_iter().map(ServerMessage::ExecConnected));
        events.extend(inner.data_connections.into_iter().map(ServerMessage::DataConnected));
        events.extend(inner.detached_connections.into_iter().map(ServerMessage::Detached));
        events.extend(self.paste_routes(&snippet.wire_routes, &imported));
        events.extend(self.paste_frames(&snippet.frames, sheet));
        let tagged = events.into_iter().map(|e| (Some(sheet), e)).collect();
        Ok((tagged, Some(ids)))
    }

    fn paste_settings_on_sheet(&mut self, id: NodeId, type_name: &str, properties: Vec<(String, DataValue)>, sheet: SheetId) -> Result<Applied, UiMessage> {
        self.require_session_node(id, sheet)?;
        let events = self.paste_settings(id, type_name, properties)?;
        Ok((events.into_iter().map(|e| (Some(sheet), e)).collect(), None))
    }

    fn set_property(&mut self, id: NodeId, name: &str, value: DataValue, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        let before = self.engine.node_model(id);
        let removed = self.engine.set_node_property(id, name, value).map_err(|e| e.ui_message())?;
        Ok(self.updated_events(id, before, removed))
    }

    fn set_pin_split(&mut self, node: NodeId, pin: PinId, split: bool, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(node, sheet)?;
        let before = self.engine.node_model(node);
        let removed = self.engine.set_pin_split(node, pin, split).map_err(|e| e.ui_message())?;
        Ok(self.updated_events(node, before, removed))
    }

    fn set_input_value(&mut self, id: NodeId, pin: PinId, value: Option<DataValue>, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
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

    fn disconnect_exec(&mut self, c: ConnectionModel, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        if !self.engine.has_node(c.src_node) || !self.engine.has_node(c.dst_node) {
            return Ok(vec![]);
        }
        self.require_session_connection(&c, sheet)?;
        Ok(if self.engine.disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin) { vec![ServerMessage::ExecDisconnected(c)] } else { vec![] })
    }

    fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        if !self.engine.has_node(dst_node) {
            return Ok(vec![]);
        }
        self.require_session_node(dst_node, sheet)?;
        Ok(if self.engine.disconnect_data(dst_node, dst_pin) { vec![ServerMessage::DataDisconnected { dst_node, dst_pin }] } else { vec![] })
    }

    fn connect_exec(&mut self, c: ConnectionModel, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_connection(&c, sheet)?;
        if self.engine.exec_connections().contains(&c) { return Ok(vec![]); }
        self.engine.connect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
        Ok(vec![ServerMessage::ExecConnected(c)])
    }

    fn connect_data(&mut self, c: ConnectionModel, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_connection(&c, sheet)?;
        if self.engine.data_connections().contains(&c) { return Ok(vec![]); }
        let replaced = self.engine.data_connections().iter().any(|old| old.dst_node == c.dst_node && old.dst_pin == c.dst_pin);
        self.engine.connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin).map_err(|e| e.ui_message())?;
        let mut events = Vec::new();
        if replaced { events.push(ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin }); }
        events.push(ServerMessage::DataConnected(c));
        Ok(events)
    }

    fn trigger(&mut self, id: NodeId, pin: PinId, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        self.engine.trigger_node(id, pin);
        Ok(vec![])
    }

    fn configure_plugin(&mut self, name: &str, values: Vec<(String, ticknode_shared::SettingUpdate)>) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        self.plugins.configure(&mut self.engine, name, values)?;
        Ok(self.plugins.info(&self.engine, name).map(|i| (None, ServerMessage::PluginUpdated(i))).into_iter().collect())
    }

    fn set_plugin_enabled(&mut self, name: &str, enabled: bool, reply: &mpsc::UnboundedSender<ServerMessage>) -> Result<Vec<(Option<SheetId>, ServerMessage)>, UiMessage> {
        let outcome = self.plugins.set_enabled(&mut self.engine, name, enabled)?;
        self.histories.clear();
        if let Some(warning) = outcome.warning {
            let _ = reply.send(ServerMessage::Error(warning));
        }
        let mut events = vec![(None, self.catalog())];
        for id in &outcome.change.replaced {
            if let Some(inst) = self.instance(*id) {
                let sheet = self.node_sheet.get(id).copied();
                events.push((sheet, ServerMessage::NodeUpdated(inst)));
            }
        }
        if !outcome.change.replaced.is_empty() || !outcome.change.detached.is_empty() || !outcome.change.removed.exec.is_empty() || !outcome.change.removed.data.is_empty() {
            self.dirty_since.get_or_insert_with(Instant::now);
            self.dirty_sheets.extend(self.sheets.iter().map(|s| s.id));
        }
        for c in outcome.change.removed.exec {
            let sheet = self.node_sheet.get(&c.src_node).copied();
            events.push((sheet, ServerMessage::ExecDisconnected(c)));
        }
        for c in outcome.change.removed.data {
            let sheet = self.node_sheet.get(&c.dst_node).copied();
            events.push((sheet, ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin }));
        }
        for d in outcome.change.detached {
            let sheet = self.node_sheet.get(&d.connection.src_node).copied();
            events.push((sheet, ServerMessage::Detached(d)));
        }
        if let Some(info) = self.plugins.info(&self.engine, name) {
            events.push((None, ServerMessage::PluginUpdated(info)));
        }
        for sheet in &self.sheets {
            events.push((Some(sheet.id), ServerMessage::History(ticknode_shared::HistoryState::default())));
        }
        Ok(events)
    }
}

pub(super) fn removed_events(removed: RemovedConnections) -> Vec<ServerMessage> {
    let exec = removed.exec.into_iter().map(ServerMessage::ExecDisconnected);
    let data = removed.data.into_iter().map(|c| ServerMessage::DataDisconnected { dst_node: c.dst_node, dst_pin: c.dst_pin });
    exec.chain(data).collect()
}
