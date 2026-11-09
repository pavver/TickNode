//! Idempotent frame updates on authoritative graph events.

use std::collections::BTreeMap;
use ticknode_shared::{Frame, FrameId, ServerMessage};

pub(super) fn applies(event: &ServerMessage) -> bool {
    matches!(event, ServerMessage::Init { .. } | ServerMessage::FrameSet(_) | ServerMessage::FrameRemoved(_))
}

pub(super) fn apply(all: &mut BTreeMap<FrameId, Frame>, event: &ServerMessage) {
    match event {
        ServerMessage::Init { frames, .. } => {
            *all = frames.iter().cloned().map(|f| (f.id, f)).collect();
        }
        ServerMessage::FrameSet(frame) => {
            all.insert(frame.id, frame.clone());
        }
        ServerMessage::FrameRemoved(id) => {
            all.remove(id);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
