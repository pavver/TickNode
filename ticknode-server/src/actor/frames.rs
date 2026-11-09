//! Frame validation, storage, layout reconciliation and clipboard import.

use std::collections::BTreeMap;
use ticknode_shared::{Frame, FrameId, ServerMessage, UiMessage, MAX_FRAMES};

use super::Actor;

impl Actor {
    pub(super) fn set_frame(&mut self, frame: Frame) -> Result<Vec<ServerMessage>, UiMessage> {
        let valid = frame.validated().ok_or_else(|| {
            UiMessage::new("error-frame-invalid", "Frame position and size must be finite")
        })?;
        if !self.frames.contains_key(&valid.id) && self.frames.len() >= MAX_FRAMES {
            return Err(UiMessage::new("error-frame-limit", "Too many frames"));
        }
        if self.frames.get(&valid.id) == Some(&valid) {
            return Ok(vec![]);
        }
        self.frames.insert(valid.id, valid.clone());
        Ok(vec![ServerMessage::FrameSet(valid)])
    }

    pub(super) fn remove_frame(&mut self, id: FrameId) -> Vec<ServerMessage> {
        if self.frames.remove(&id).is_some() {
            vec![ServerMessage::FrameRemoved(id)]
        } else {
            vec![]
        }
    }

    pub(super) fn restore_frames(&mut self, target: &[Frame]) -> Vec<ServerMessage> {
        let target_map: BTreeMap<FrameId, Frame> = target
            .iter()
            .filter_map(|f| f.clone().validated())
            .map(|f| (f.id, f))
            .collect();
        let mut events = Vec::new();
        for id in self.frames.keys().copied().collect::<Vec<_>>() {
            if !target_map.contains_key(&id) {
                events.push(ServerMessage::FrameRemoved(id));
            }
        }
        for (id, frame) in &target_map {
            if self.frames.get(id) != Some(frame) {
                events.push(ServerMessage::FrameSet(frame.clone()));
            }
        }
        self.frames = target_map;
        events
    }

    pub(super) fn paste_frames(&mut self, frames: &[Frame]) -> Vec<ServerMessage> {
        let mut events = Vec::new();
        for frame in frames {
            if self.frames.len() >= MAX_FRAMES {
                break;
            }
            if let Some(mut valid) = frame.clone().validated() {
                valid.id = FrameId::generate();
                self.frames.insert(valid.id, valid.clone());
                events.push(ServerMessage::FrameSet(valid));
            }
        }
        events
    }
}
