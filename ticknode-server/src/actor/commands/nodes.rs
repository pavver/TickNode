use super::*;
use ticknode_shared::ConnectionEnd;

impl Actor {
    pub(super) fn add_node(
        &mut self,
        type_name: &str,
        position: Position,
        sheet: SheetId,
    ) -> Result<Applied, UiMessage> {
        let id = NodeId::generate();
        self.engine
            .create_node(id, type_name, &[])
            .map_err(|e| e.ui_message())?;
        self.node_sheet.insert(id, sheet);
        self.engine.set_node_sheet(id, Some(sheet));
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(std::iter::once(id), true);
        }
        self.positions.insert(id, position);
        Ok((
            vec![(
                Some(sheet),
                ServerMessage::NodeAdded(self.instance(id).expect("the node was just created")),
            )],
            Some(vec![id]),
        ))
    }

    pub(super) fn add_node_with(
        &mut self,
        type_name: &str,
        position: Position,
        properties: Vec<(String, DataValue)>,
        sheet: SheetId,
    ) -> Result<Applied, UiMessage> {
        for (k, _) in &properties {
            if k.starts_with("in:") || k.starts_with("split:") {
                return Err(UiMessage::new(
                    "error-invalid-property",
                    format!("Invalid property: {k}"),
                )
                .arg("property", k));
            }
        }
        let id = NodeId::generate();
        self.engine
            .create_node_interactive(id, type_name, &properties)
            .map_err(|e| e.ui_message())?;
        self.node_sheet.insert(id, sheet);
        self.engine.set_node_sheet(id, Some(sheet));
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(std::iter::once(id), true);
        }
        self.positions.insert(id, position);
        Ok((
            vec![(
                Some(sheet),
                ServerMessage::NodeAdded(self.instance(id).expect("the node was just created")),
            )],
            Some(vec![id]),
        ))
    }

    pub(super) fn add_node_connected(
        &mut self,
        type_name: &str,
        position: Position,
        from: ConnectionEnd,
        pin: PinId,
        sheet: SheetId,
    ) -> Result<Applied, UiMessage> {
        self.require_session_node(from.node, sheet)?;
        let definition = self
            .engine
            .node_definition(from.node)
            .expect("the node exists");
        let origin = definition
            .pins
            .iter()
            .find(|p| p.id == from.pin)
            .ok_or_else(|| ticknode_core::EngineError::InvalidPinConnection.ui_message())?;
        let (mut events, created) = self.add_node(type_name, position, sheet)?;
        let id = created.as_ref().expect("add_node returns an ID")[0];

        let new_definition = self
            .engine
            .node_definition(id)
            .expect("the node was just created");
        let origin = definition
            .pins
            .iter()
            .find(|p| {
                p.id == from.pin
                    && new_definition.pins.iter().any(|n| {
                        n.id == pin
                            && n.direction != p.direction
                            && matches!(
                                (&p.pin_type, &n.pin_type),
                                (PinType::Exec, PinType::Exec)
                                    | (PinType::Data(_), PinType::Data(_))
                            )
                    })
            })
            .unwrap_or(origin);
        if new_definition.adaptive
            && let PinType::Data(data_type) = &origin.pin_type
        {
            if let Err(error) = self.engine.adapt_node(id, pin, data_type) {
                self.engine.remove_node(id);
                self.node_sheet.remove(&id);
                self.positions.remove(&id);
                self.sizes.remove(&id);
                return Err(error.ui_message());
            }
            events[0] = (
                Some(sheet),
                ServerMessage::NodeAdded(self.instance(id).expect("the node exists")),
            );
        }
        let c = if origin.direction == PinDirection::Output {
            ConnectionModel {
                src_node: from.node,
                src_pin: from.pin,
                dst_node: id,
                dst_pin: pin,
            }
        } else {
            ConnectionModel {
                src_node: id,
                src_pin: pin,
                dst_node: from.node,
                dst_pin: from.pin,
            }
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

    pub(super) fn remove_node(
        &mut self,
        id: NodeId,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        let detached: Vec<_> = self
            .engine
            .detached_connections()
            .iter()
            .filter(|d| d.connection.src_node == id || d.connection.dst_node == id)
            .copied()
            .collect();
        self.engine.remove_node(id);
        self.node_sheet.remove(&id);
        self.positions.remove(&id);
        self.sizes.remove(&id);
        self.traces.remove(&id);
        let mut events: Vec<_> = detached
            .into_iter()
            .map(ServerMessage::DetachedRemoved)
            .collect();
        events.push(ServerMessage::NodeRemoved(id));
        Ok(events)
    }

    pub(super) fn move_node(
        &mut self,
        id: NodeId,
        position: Position,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        if self.positions.get(&id).copied().unwrap_or_default() == position {
            return Ok(vec![]);
        }
        self.positions.insert(id, position);
        Ok(vec![ServerMessage::NodeMoved { id, position }])
    }

    pub(super) fn paste(
        &mut self,
        snippet: ticknode_shared::GraphModel,
        sheet: SheetId,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Applied, UiMessage> {
        if snippet.nodes.len() > MAX_PASTE_NODES {
            return Err(UiMessage::new(
                "error-paste-too-many",
                format!(
                    "Too many nodes to paste: {} (maximum {})",
                    snippet.nodes.len(),
                    MAX_PASTE_NODES
                ),
            )
            .arg("count", snippet.nodes.len())
            .arg("max", MAX_PASTE_NODES));
        }
        let (imported, warnings) = self.engine.import_snippet(&snippet);
        if !warnings.is_empty() {
            let details = warnings.join("; ");
            let _ = reply.send(ServerMessage::Error(
                UiMessage::new(
                    "error-paste-incomplete",
                    format!("Not everything was pasted: {details}"),
                )
                .arg("details", details),
            ));
        }
        let positions: HashMap<NodeId, Position> =
            snippet.nodes.iter().map(|n| (n.id, n.position)).collect();
        let ids: Vec<NodeId> = imported.iter().map(|(_, new)| *new).collect();
        for (old, new) in &imported {
            self.node_sheet.insert(*new, sheet);
            self.engine.set_node_sheet(*new, Some(sheet));
            self.positions
                .insert(*new, positions.get(old).copied().unwrap_or_default());
            if let Some(size) = snippet
                .nodes
                .iter()
                .find(|n| n.id == *old)
                .and_then(|n| n.size)
                .and_then(NodeSize::validated)
            {
                self.sizes.insert(*new, size);
            }
        }
        if !self.sheet_is_enabled(sheet) {
            self.engine.set_disabled(ids.iter().copied(), true);
        }
        let mut events: Vec<ServerMessage> = ids
            .iter()
            .filter_map(|id| self.instance(*id))
            .map(ServerMessage::NodeAdded)
            .collect();
        let inner = self.engine.export_snippet(&ids);
        events.extend(
            inner
                .exec_connections
                .into_iter()
                .map(ServerMessage::ExecConnected),
        );
        events.extend(
            inner
                .data_connections
                .into_iter()
                .map(ServerMessage::DataConnected),
        );
        events.extend(
            inner
                .detached_connections
                .into_iter()
                .map(ServerMessage::Detached),
        );
        events.extend(self.paste_routes(&snippet.wire_routes, &imported));
        events.extend(self.paste_frames(&snippet.frames, sheet));
        let tagged = events.into_iter().map(|e| (Some(sheet), e)).collect();
        Ok((tagged, Some(ids)))
    }

    pub(super) fn paste_settings_on_sheet(
        &mut self,
        id: NodeId,
        type_name: &str,
        properties: Vec<(String, DataValue)>,
        sheet: SheetId,
    ) -> Result<Applied, UiMessage> {
        self.require_session_node(id, sheet)?;
        let events = self.paste_settings(id, type_name, properties)?;
        Ok((events.into_iter().map(|e| (Some(sheet), e)).collect(), None))
    }

    pub(super) fn trigger(
        &mut self,
        id: NodeId,
        pin: PinId,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        self.engine.trigger_node(id, pin);
        Ok(vec![])
    }
}
