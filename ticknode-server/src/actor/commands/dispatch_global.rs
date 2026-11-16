use super::*;
use std::ops::ControlFlow;

type Handler = fn(
    &mut Actor,
    ClientMessage,
    &mpsc::UnboundedSender<ServerMessage>,
) -> Result<Dispatch, UiMessage>;

impl Actor {
    pub(super) fn apply_global_command(
        &mut self,
        mut msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        let handlers: &[Handler] = &[
            Self::plugin_commands,
            Self::create_sheet_commands,
            Self::edit_sheet_commands,
            Self::variable_commands,
            Self::session_commands,
        ];
        for handler in handlers {
            match handler(self, msg, reply)? {
                ControlFlow::Break(applied) => return Ok(ControlFlow::Break(applied)),
                ControlFlow::Continue(next) => msg = next,
            }
        }
        Ok(ControlFlow::Continue(msg))
    }
    fn plugin_commands(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        match msg {
            ClientMessage::SetServerSettings(settings) => {
                let events = self.set_server_settings(settings)?;
                Ok(ControlFlow::Break((events, None)))
            }
            ClientMessage::SetPluginEnabled { name, enabled } => {
                let events = self.set_plugin_enabled(&name, enabled, reply)?;
                Ok(ControlFlow::Break((events, None)))
            }
            ClientMessage::ConfigurePlugin { name, values } => {
                let events = self.configure_plugin(&name, values)?;
                Ok(ControlFlow::Break((events, None)))
            }
            ClientMessage::GetUsage => {
                let report = self.engine.usage_report(chrono::Utc::now().timestamp());
                let _ = reply.send(ServerMessage::Usage(report));
                Ok(ControlFlow::Break((vec![], None)))
            }
            msg => Ok(ControlFlow::Continue(msg)),
        }
    }
    fn create_sheet_commands(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        match msg {
            ClientMessage::CreateSheet { title, caption } => {
                let _id = self.create_sheet(title, caption, reply)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::DuplicateSheet(id) => {
                let _new_id = self.duplicate_sheet(id, reply)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            msg => Ok(ControlFlow::Continue(msg)),
        }
    }
    fn edit_sheet_commands(
        &mut self,
        msg: ClientMessage,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        match msg {
            ClientMessage::UpdateSheet(info) => {
                self.update_sheet(info)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::DeleteSheet(id) => {
                self.delete_sheet(id)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::MoveSheet { id, index } => {
                self.move_sheet(id, index)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            msg => Ok(ControlFlow::Continue(msg)),
        }
    }
    fn variable_commands(
        &mut self,
        msg: ClientMessage,
        _reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        match msg {
            ClientMessage::CreateVariable(var) => {
                self.create_variable(var)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::SetVariable { key, value } => {
                self.set_variable(key, value)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::DeleteVariable(key) => {
                self.delete_variable(key)?;
                Ok(ControlFlow::Break((vec![], None)))
            }
            msg => Ok(ControlFlow::Continue(msg)),
        }
    }
    fn session_commands(
        &mut self,
        msg: ClientMessage,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Dispatch, UiMessage> {
        match msg {
            ClientMessage::OpenSheet(target) => {
                if let Some(id) = target
                    && !self.sheets.iter().any(|s| s.id == id)
                {
                    return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                }
                let _ = reply.send(self.snapshot(target));
                Ok(ControlFlow::Break((vec![], None)))
            }
            ClientMessage::Undo | ClientMessage::Redo | ClientMessage::Batch(_) => {
                Err(super::history::batch_error())
            }
            msg => Ok(ControlFlow::Continue(msg)),
        }
    }
}
