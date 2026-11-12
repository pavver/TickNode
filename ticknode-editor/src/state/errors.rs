//! Runtime error snapshots and idempotent updates, separate from node lifetimes.

use std::collections::BTreeMap;
use ticknode_shared::{NodeError, NodeId, ServerMessage};

pub type NodeErrors = BTreeMap<NodeId, ClientError>;

#[derive(Debug, Clone, PartialEq)]
pub struct ClientError {
    pub error: NodeError,
    pub received_at: f64,
}

impl ClientError {
    pub fn age_ms(&self, now: f64) -> f64 {
        self.error.age_ms as f64 + (now - self.received_at).max(0.0)
    }
}

pub(super) fn applies(msg: &ServerMessage) -> bool {
    matches!(
        msg,
        ServerMessage::Init { .. }
            | ServerMessage::NodeErrors { .. }
            | ServerMessage::NodeRemoved(_)
    )
}

pub(super) fn apply(all: &mut NodeErrors, msg: &ServerMessage, now: f64) {
    let set = match msg {
        ServerMessage::Init { node_errors, .. } => {
            all.clear();
            node_errors
        }
        ServerMessage::NodeErrors { set, cleared } => {
            for id in cleared {
                all.remove(id);
            }
            set
        }
        ServerMessage::NodeRemoved(id) => {
            all.remove(id);
            return;
        }
        _ => return,
    };
    for error in set {
        if all.get(&error.node).is_some_and(|old| old.error == *error) {
            continue;
        }
        all.insert(
            error.node,
            ClientError {
                error: error.clone(),
                received_at: now,
            },
        );
    }
}

#[cfg(test)]
mod tests;
