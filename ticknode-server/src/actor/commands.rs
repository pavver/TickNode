use super::*;
use ticknode_shared::{ConnectionModel, DataValue, PinDirection, PinId, PinType, SheetId};

mod connections;
mod nodes;

pub(super) use connections::removed_events;

type Applied = (Vec<(Option<SheetId>, ServerMessage)>, Option<Vec<NodeId>>);

impl Actor {
    pub(super) fn require_session_node(&self, id: NodeId, sheet: SheetId) -> Result<(), UiMessage> {
        if self.engine.has_node(id) && self.node_sheet.get(&id) == Some(&sheet) {
            Ok(())
        } else {
            Err(EngineError::NodeNotFound(id).ui_message())
        }
    }

    pub(super) fn sheet_is_enabled(&self, sheet: SheetId) -> bool {
        self.sheets
            .iter()
            .find(|s| s.id == sheet)
            .map_or(true, |s| s.enabled)
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
            ClientMessage::SetServerSettings(settings) => {
                let events = self.set_server_settings(settings)?;
                return Ok((events, None));
            }
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
            ClientMessage::CreateVariable(var) => {
                self.create_variable(var)?;
                return Ok((vec![], None));
            }
            ClientMessage::SetVariable { key, value } => {
                self.set_variable(key, value)?;
                return Ok((vec![], None));
            }
            ClientMessage::DeleteVariable(key) => {
                self.delete_variable(key)?;
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
            ClientMessage::Undo | ClientMessage::Redo | ClientMessage::Batch(_) => {
                return Err(super::history::batch_error());
            }
            _ => {}
        }

        // All remaining commands require an open sheet
        let s = sheet
            .or_else(|| self.sheets.first().map(|sh| sh.id))
            .ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
        if !self.sheets.iter().any(|sh| sh.id == s) {
            return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
        }

        let only =
            |events: Vec<ServerMessage>| (events.into_iter().map(|e| (Some(s), e)).collect(), None);
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. });

        let result: Applied = match msg {
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
            ClientMessage::ConfirmPins(id) => {
                self.require_session_node(id, s)?;
                only(
                    if self.engine.confirm_pins(id).map_err(|e| e.ui_message())? {
                        vec![ServerMessage::NodeUpdated(
                            self.instance(id).expect("the node exists"),
                        )]
                    } else {
                        vec![]
                    },
                )
            }
            ClientMessage::RemoveDetached(d) => {
                self.require_session_node(d.connection.src_node, s)?;
                only(if self.engine.remove_detached(&d) {
                    vec![ServerMessage::DetachedRemoved(d)]
                } else {
                    vec![]
                })
            }
            ClientMessage::RemoveNode(id) => only(self.remove_node(id, s)?),
            ClientMessage::SetWireRoute(route) => only(self.set_wire_route(route, s)?),
            ClientMessage::MoveNode { id, position } => only(self.move_node(id, position, s)?),
            ClientMessage::ResizeNode { id, size } => only(self.resize_node(id, size, s)?),
            ClientMessage::SetFrame(frame) => only(self.set_frame(frame, s)?),
            ClientMessage::RemoveFrame(id) => only(self.remove_frame(id, s)),
            ClientMessage::Paste(snippet) => self.paste(snippet, s, reply)?,
            ClientMessage::PasteSettings {
                id,
                type_name,
                properties,
            } => self.paste_settings_on_sheet(id, &type_name, properties, s)?,
            ClientMessage::SetProperty { id, name, value } => {
                match self.set_property(id, &name, value, s) {
                    Ok(events) => only(events),
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
                only(self.set_pin_split(node, pin, split, s)?)
            }
            ClientMessage::SetInputValue { id, pin, value } => {
                only(self.set_input_value(id, pin, value, s)?)
            }
            ClientMessage::ConnectExec(c) => only(self.connect_exec(c, s)?),
            ClientMessage::DisconnectExec(c) => only(self.disconnect_exec(c, s)?),
            ClientMessage::ConnectData(c) => only(self.connect_data(c, s)?),
            ClientMessage::DisconnectData { dst_node, dst_pin } => {
                only(self.disconnect_data(dst_node, dst_pin, s)?)
            }
            ClientMessage::TriggerNode { id, pin } => only(self.trigger(id, pin, s)?),
            _ => unreachable!(),
        };

        if mutates && !result.0.is_empty() {
            self.dirty_sheets.insert(s);
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok(result)
    }
}
