//! Authoritative delay snapshots with local timestamps for smooth CSS countdowns.

use std::collections::BTreeMap;
use ticknode_shared::{NodeId, NodeTimer, ServerMessage};

pub type NodeTimers = BTreeMap<NodeId, ClientTimer>;

#[derive(Debug, Clone, PartialEq)]
pub struct ClientTimer {
    pub timer: NodeTimer,
    pub received_at: f64,
}

impl ClientTimer {
    /// Remaining milliseconds and fraction at mount time, accounting for time spent offscreen.
    pub fn progress(&self, now: f64) -> (f64, f64) {
        let remaining = (self.timer.remaining_ms as f64 - (now - self.received_at).max(0.0)).max(0.0);
        let fraction = if self.timer.total_ms == 0 { 0.0 } else { (remaining / self.timer.total_ms as f64).min(1.0) };
        (remaining, fraction)
    }
}

pub(super) fn applies(msg: &ServerMessage) -> bool {
    matches!(msg, ServerMessage::Init { .. } | ServerMessage::Timers(_) | ServerMessage::NodeRemoved(_))
}

pub(super) fn apply(all: &mut NodeTimers, msg: &ServerMessage, now: f64) {
    let timers = match msg {
        ServerMessage::Init { timers, .. } | ServerMessage::Timers(timers) => timers,
        ServerMessage::NodeRemoved(id) => { all.remove(id); return; }
        _ => return,
    };
    *all = timers.iter().map(|timer| {
        // Remaining time is freshly measured in every snapshot. A restarted delay can have
        // exactly the same values as its predecessor, so it still needs a new animation.
        (timer.node, ClientTimer { timer: timer.clone(), received_at: now })
    }).collect();
}

#[cfg(test)]
mod tests;
