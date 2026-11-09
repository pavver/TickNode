//! Runtime node errors are independent of log throttling and graph persistence.

use std::collections::{HashMap, HashSet};
use std::time::Instant;
use ticknode_shared::{NodeError, NodeId, UiMessage};
use crate::TickEngine;

#[derive(Default)]
pub(super) struct NodeErrors {
    current: HashMap<NodeId, (UiMessage, Instant, u32)>,
    changed: HashSet<NodeId>,
}

impl TickEngine {
    /// Current errors with their age at the time of the snapshot.
    pub fn node_errors(&self) -> Vec<NodeError> {
        let mut errors: Vec<_> = self.node_errors.current.iter().map(|(&node, (message, at, count))| NodeError {
            node, message: message.clone(), age_ms: elapsed_ms(*at), count: *count,
        }).collect();
        errors.sort_by_key(|e| e.node);
        errors
    }

    /// Deduplicated changes; refreshes are sent at most once per telemetry flush.
    pub fn take_node_error_changes(&mut self) -> (Vec<NodeError>, Vec<NodeId>) {
        self.collect_node_errors();
        let changed = std::mem::take(&mut self.node_errors.changed);
        let mut set: Vec<_> = changed.iter().filter_map(|&node| {
            let (message, at, count) = self.node_errors.current.get(&node)?;
            Some(NodeError { node, message: message.clone(), age_ms: elapsed_ms(*at), count: *count })
        }).collect();
        set.sort_by_key(|e| e.node);
        let mut cleared: Vec<_> = changed.into_iter().filter(|id| !self.node_errors.current.contains_key(id)).collect();
        cleared.sort();
        (set, cleared)
    }

    pub(crate) fn node_succeeded(&mut self, id: NodeId) { self.successful.insert(id); }

    pub(crate) fn clear_node_error(&mut self, id: NodeId) {
        self.host_state.errors.retain(|(node, ..)| *node != id);
        self.successful.remove(&id);
        if self.node_errors.current.remove(&id).is_some() {
            self.node_errors.changed.insert(id);
        }
        self.required_reported.remove(&id);
        self.frozen_reported.remove(&id);
    }

    pub(crate) fn collect_node_errors(&mut self) {
        let mut failed = HashSet::new();
        for (id, message, at) in std::mem::take(&mut self.host_state.errors) {
            if !self.has_node(id) || id.0.is_nil() { continue; }
            failed.insert(id);
            let count = self.node_errors.current.get(&id)
                .filter(|(old, ..)| *old == message).map_or(1, |(_, _, n)| n.saturating_add(1));
            self.node_errors.current.insert(id, (message, at, count));
            self.node_errors.changed.insert(id);
        }
        for id in std::mem::take(&mut self.successful) {
            if !failed.contains(&id) { self.clear_node_error(id); }
        }
    }
}

fn elapsed_ms(at: Instant) -> u64 {
    at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests;
