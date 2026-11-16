use super::*;
use ticknode_shared::{ConnectionModel, DataValue, PinDirection, PinId, PinType, SheetId};

mod connections;
mod dispatch_global;
mod dispatch_sheet;
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
            .is_none_or(|s| s.enabled)
    }

    /// Applies an editor command; returns events to broadcast and the IDs of nodes created.
    pub(super) fn apply_command(
        &mut self,
        msg: ClientMessage,
        sheet: Option<SheetId>,
        reply: &mpsc::UnboundedSender<ServerMessage>,
    ) -> Result<Applied, UiMessage> {
        let msg = match self.apply_global_command(msg, reply)? {
            std::ops::ControlFlow::Break(applied) => return Ok(applied),
            std::ops::ControlFlow::Continue(msg) => msg,
        };
        let sheet = self.command_sheet(sheet)?;
        let mutates = !matches!(msg, ClientMessage::TriggerNode { .. });
        let result = match self.apply_sheet_command(msg, sheet, reply)? {
            std::ops::ControlFlow::Break(applied) => applied,
            std::ops::ControlFlow::Continue(_) => return Err(super::history::batch_error()),
        };
        self.mark_command_changes(sheet, mutates, !result.0.is_empty());
        Ok(result)
    }

    fn mark_command_changes(&mut self, sheet: SheetId, mutates: bool, has_events: bool) {
        if mutates && has_events {
            self.dirty_sheets.insert(sheet);
            self.dirty_since.get_or_insert_with(Instant::now);
        }
    }

    fn command_sheet(&self, sheet: Option<SheetId>) -> Result<SheetId, UiMessage> {
        let sheet = sheet
            .or_else(|| self.sheets.first().map(|sh| sh.id))
            .ok_or_else(|| UiMessage::new("error-no-sheet", "Open a sheet first"))?;
        if !self.sheets.iter().any(|sh| sh.id == sheet) {
            return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
        }
        Ok(sheet)
    }
}

type Dispatch = std::ops::ControlFlow<Applied, ClientMessage>;

fn sheet_events(sheet: SheetId, events: Vec<ServerMessage>) -> Applied {
    (
        events
            .into_iter()
            .map(|event| (Some(sheet), event))
            .collect(),
        None,
    )
}
