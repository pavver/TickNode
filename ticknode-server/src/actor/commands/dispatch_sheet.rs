use super::*;
use std::ops::ControlFlow;

type Handler = fn(
    &mut Actor,
    ClientMessage,
    SheetId,
    &mpsc::UnboundedSender<ServerMessage>,
) -> Result<Dispatch, UiMessage>;

impl Actor {
    pub(super) fn apply_sheet_command(
        &mut self,
        mut msg: ClientMessage,
        s: SheetId,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let handlers: &[Handler] = &[
            Self::create_nodes,
            Self::confirm_node_pins,
            Self::remove_detached_wire,
            Self::remove_nodes,
            Self::node_layout,
            Self::frames,
            Self::paste_nodes,
            Self::node_properties,
            Self::input_values,
            Self::exec_connections,
            Self::data_connections,
        ];
        for handler in handlers {
            match handler(self, msg, s, reply)? {
                ControlFlow::Break(applied) => return Ok(ControlFlow::Break(applied)),
                ControlFlow::Continue(next) => msg = next,
            }
        }
        Ok(ControlFlow::Continue(msg))
    }
    fn create_nodes(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::AddNode {
                type_name,
                position,
            } => self.add_node(&type_name, position, s)?,
            ClientMessage::AddNodeWith {
                type_name,
                position,
                properties,
            } => self.add_node_with(&type_name, position, properties, s)?,
            ClientMessage::AddNodeConnected {
                type_name,
                position,
                from,
                pin,
            } => self.add_node_connected(&type_name, position, from, pin, s)?,
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn confirm_node_pins(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::ConfirmPins(id) => {
                self.require_session_node(id, s)?;
                sheet_events(
                    s,
                    if self.engine.confirm_pins(id).map_err(|e| e.ui_message())? {
                        vec![ServerMessage::NodeUpdated(
                            self.instance(id)
                                .ok_or_else(|| EngineError::NodeNotFound(id).ui_message())?,
                        )]
                    } else {
                        vec![]
                    },
                )
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn remove_detached_wire(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::RemoveDetached(d) => {
                self.require_session_node(d.connection.src_node, s)?;
                sheet_events(
                    s,
                    if self.engine.remove_detached(&d) {
                        vec![ServerMessage::DetachedRemoved(d)]
                    } else {
                        vec![]
                    },
                )
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn remove_nodes(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::RemoveNode(id) => sheet_events(s, self.remove_node(id, s)?),
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn node_layout(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::SetWireRoute(route) => sheet_events(s, self.set_wire_route(route, s)?),
            ClientMessage::MoveNode { id, position } => {
                sheet_events(s, self.move_node(id, position, s)?)
            }
            ClientMessage::ResizeNode { id, size } => {
                sheet_events(s, self.resize_node(id, size, s)?)
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn frames(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::SetFrame(frame) => sheet_events(s, self.set_frame(frame, s)?),
            ClientMessage::RemoveFrame(id) => sheet_events(s, self.remove_frame(id, s)),
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn paste_nodes(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::Paste(snippet) => self.paste(snippet, s, reply)?,
            ClientMessage::PasteSettings {
                id,
                type_name,
                properties,
            } => self.paste_settings_on_sheet(id, &type_name, properties, s)?,
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn node_properties(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::SetProperty { id, name, value } => {
                match self.set_property(id, &name, value, s) {
                    Ok(events) => sheet_events(s, events),
                    Err(error) => {
                        if self.node_sheet.get(&id) == Some(&s)
                            && let Some(node) = self.instance(id)
                        {
                            let _ = reply.send(ServerMessage::NodeUpdated(node));
                        }
                        return Err(error);
                    }
                }
            }
            ClientMessage::SetPinSplit { node, pin, split } => {
                sheet_events(s, self.set_pin_split(node, pin, split, s)?)
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn input_values(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::SetInputValue { id, pin, value } => {
                sheet_events(s, self.set_input_value(id, pin, value, s)?)
            }
            ClientMessage::SetInputVariable { id, pin, variable } => {
                sheet_events(s, self.set_input_variable(id, pin, variable, s)?)
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn exec_connections(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::ConnectExec(c) => sheet_events(s, self.connect_exec(c, s)?),
            ClientMessage::DisconnectExec(c) => sheet_events(s, self.disconnect_exec(c, s)?),
            ClientMessage::TriggerNode { id, pin } => sheet_events(s, self.trigger(id, pin, s)?),
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
    fn data_connections(
        &mut self,
        msg: ClientMessage,
        s: SheetId,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let applied = match msg {
            ClientMessage::ConnectData(c) => sheet_events(s, self.connect_data(c, s)?),
            ClientMessage::DisconnectData { dst_node, dst_pin } => {
                sheet_events(s, self.disconnect_data(dst_node, dst_pin, s)?)
            }
            msg => return Ok(ControlFlow::Continue(msg)),
        };
        Ok(ControlFlow::Break(applied))
    }
}
