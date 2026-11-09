//! Frame validation, storage, layout reconciliation and clipboard import.

use std::collections::BTreeMap;
use ticknode_shared::{Frame, FrameId, ServerMessage, SheetId, UiMessage, MAX_FRAMES};

use super::Actor;

impl Actor {
    pub(super) fn set_frame(&mut self, frame: Frame, sheet: SheetId) -> Result<Vec<ServerMessage>, UiMessage> {
        let valid = frame.validated().ok_or_else(|| {
            UiMessage::new("error-frame-invalid", "Frame position and size must be finite")
        })?;
        if self.frame_sheet.get(&valid.id).is_some_and(|s| *s != sheet) {
            return Err(UiMessage::new("error-no-sheet", "Open a sheet first"));
        }
        if !self.frames.contains_key(&valid.id) && self.frames.len() >= MAX_FRAMES {
            return Err(UiMessage::new("error-frame-limit", "Too many frames"));
        }
        if self.frames.get(&valid.id) == Some(&valid) {
            return Ok(vec![]);
        }
        self.frame_sheet.insert(valid.id, sheet);
        self.frames.insert(valid.id, valid.clone());
        Ok(vec![ServerMessage::FrameSet(valid)])
    }

    pub(super) fn remove_frame(&mut self, id: FrameId, sheet: SheetId) -> Vec<ServerMessage> {
        if self.frame_sheet.get(&id) != Some(&sheet) {
            return vec![];
        }
        self.frame_sheet.remove(&id);
        if self.frames.remove(&id).is_some() {
            vec![ServerMessage::FrameRemoved(id)]
        } else {
            vec![]
        }
    }

    pub(super) fn restore_frames_for_sheet(&mut self, sheet: SheetId, target: &[Frame]) -> Vec<ServerMessage> {
        let target_map: BTreeMap<FrameId, Frame> = target
            .iter()
            .filter_map(|f| f.clone().validated())
            .map(|f| (f.id, f))
            .collect();
        let mut events = Vec::new();
        let sheet_frame_ids: Vec<FrameId> = self.frame_sheet
            .iter()
            .filter(|(_, sh)| **sh == sheet)
            .map(|(&id, _)| id)
            .collect();
        for id in sheet_frame_ids {
            if !target_map.contains_key(&id) {
                self.frame_sheet.remove(&id);
                self.frames.remove(&id);
                events.push(ServerMessage::FrameRemoved(id));
            }
        }
        for (id, frame) in target_map {
            self.frame_sheet.insert(id, sheet);
            if self.frames.get(&id) != Some(&frame) {
                events.push(ServerMessage::FrameSet(frame.clone()));
                self.frames.insert(id, frame);
            }
        }
        events
    }

    pub(super) fn paste_frames(&mut self, frames: &[Frame], sheet: SheetId) -> Vec<ServerMessage> {
        let mut events = Vec::new();
        for frame in frames {
            if self.frames.len() >= MAX_FRAMES {
                break;
            }
            if let Some(mut valid) = frame.clone().validated() {
                valid.id = FrameId::generate();
                self.frame_sheet.insert(valid.id, sheet);
                self.frames.insert(valid.id, valid.clone());
                events.push(ServerMessage::FrameSet(valid));
            }
        }
        events
    }
}
