use super::*;
use ticknode_shared::ConnectionModel;

impl Actor {
    pub(in crate::actor) fn require_session_connection(
        &self,
        c: &ConnectionModel,
        sheet: SheetId,
    ) -> Result<(), UiMessage> {
        let src_sheet = self.node_sheet.get(&c.src_node).copied();
        let dst_sheet = self.node_sheet.get(&c.dst_node).copied();
        if src_sheet.is_none() {
            return Err(EngineError::NodeNotFound(c.src_node).ui_message());
        }
        if dst_sheet.is_none() {
            return Err(EngineError::NodeNotFound(c.dst_node).ui_message());
        }
        if src_sheet != dst_sheet {
            return Err(UiMessage::new(
                "error-cross-sheet",
                "Nodes on different sheets cannot be connected",
            ));
        }
        if src_sheet != Some(sheet) {
            return Err(EngineError::NodeNotFound(c.src_node).ui_message());
        }
        Ok(())
    }

    pub(super) fn set_property(
        &mut self,
        id: NodeId,
        name: &str,
        value: DataValue,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        let before = self.engine.node_model(id);
        let removed = self
            .engine
            .set_node_property(id, name, value)
            .map_err(|e| e.ui_message())?;
        Ok(self.updated_events(id, before, removed))
    }

    pub(super) fn set_pin_split(
        &mut self,
        node: NodeId,
        pin: PinId,
        split: bool,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(node, sheet)?;
        let before = self.engine.node_model(node);
        let removed = self
            .engine
            .set_pin_split(node, pin, split)
            .map_err(|e| e.ui_message())?;
        Ok(self.updated_events(node, before, removed))
    }

    pub(super) fn set_input_value(
        &mut self,
        id: NodeId,
        pin: PinId,
        value: Option<DataValue>,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_node(id, sheet)?;
        let before = self.engine.node_model(id);
        self.engine
            .set_input_value(id, pin, value)
            .map_err(|e| e.ui_message())?;
        Ok(self.updated_events(id, before, RemovedConnections::default()))
    }

    pub(super) fn updated_events(
        &self,
        id: NodeId,
        before: Option<ticknode_shared::NodeModel>,
        removed: RemovedConnections,
    ) -> Vec<ServerMessage> {
        if self.engine.node_model(id) == before {
            return vec![];
        }
        let mut events = removed_events(removed);
        events.push(ServerMessage::NodeUpdated(
            self.instance(id).expect("the node exists"),
        ));
        events
    }

    pub(super) fn disconnect_exec(
        &mut self,
        c: ConnectionModel,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        if !self.engine.has_node(c.src_node) || !self.engine.has_node(c.dst_node) {
            return Ok(vec![]);
        }
        self.require_session_connection(&c, sheet)?;
        Ok(
            if self
                .engine
                .disconnect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin)
            {
                vec![ServerMessage::ExecDisconnected(c)]
            } else {
                vec![]
            },
        )
    }

    pub(super) fn disconnect_data(
        &mut self,
        dst_node: NodeId,
        dst_pin: PinId,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        if !self.engine.has_node(dst_node) {
            return Ok(vec![]);
        }
        self.require_session_node(dst_node, sheet)?;
        Ok(if self.engine.disconnect_data(dst_node, dst_pin) {
            vec![ServerMessage::DataDisconnected { dst_node, dst_pin }]
        } else {
            vec![]
        })
    }

    pub(super) fn connect_exec(
        &mut self,
        c: ConnectionModel,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_connection(&c, sheet)?;
        if self.engine.exec_connections().contains(&c) {
            return Ok(vec![]);
        }
        self.engine
            .connect_exec(c.src_node, c.src_pin, c.dst_node, c.dst_pin)
            .map_err(|e| e.ui_message())?;
        Ok(vec![ServerMessage::ExecConnected(c)])
    }

    pub(super) fn connect_data(
        &mut self,
        c: ConnectionModel,
        sheet: SheetId,
    ) -> Result<Vec<ServerMessage>, UiMessage> {
        self.require_session_connection(&c, sheet)?;
        if self.engine.data_connections().contains(&c) {
            return Ok(vec![]);
        }
        let replaced = self
            .engine
            .data_connections()
            .iter()
            .any(|old| old.dst_node == c.dst_node && old.dst_pin == c.dst_pin);
        let before = self.instance(c.dst_node);
        self.engine
            .connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin)
            .map_err(|e| e.ui_message())?;
        let mut events = Vec::new();
        if replaced {
            events.push(ServerMessage::DataDisconnected {
                dst_node: c.dst_node,
                dst_pin: c.dst_pin,
            });
        }
        if self.instance(c.dst_node) != before {
            events.push(ServerMessage::NodeUpdated(
                self.instance(c.dst_node).expect("the node exists"),
            ));
        }
        events.push(ServerMessage::DataConnected(c));
        Ok(events)
    }
}

pub(in crate::actor) fn removed_events(removed: RemovedConnections) -> Vec<ServerMessage> {
    let exec = removed
        .exec
        .into_iter()
        .map(ServerMessage::ExecDisconnected);
    let data = removed
        .data
        .into_iter()
        .map(|c| ServerMessage::DataDisconnected {
            dst_node: c.dst_node,
            dst_pin: c.dst_pin,
        });
    exec.chain(data).collect()
}
