//! Execution telemetry for editors, sent at a fixed rate instead of after every tick.
//!
//! Ticks may be as short as 1 ms; one `Executed` message per tick would flood the editors (and the
//! broadcast channel) without showing anything a human can see. Traces of one interval are merged per
//! node (latest inputs/outputs, union of fired pins), logs are kept in order up to a limit.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ticknode_shared::{NodeId, NodeTrace, ServerMessage};

/// How often accumulated telemetry is sent to editors.
pub(super) const INTERVAL: Duration = Duration::from_millis(100);
/// Log lines sent per interval; the rest are counted and reported as one line.
pub(super) const MAX_LOGS: usize = 1000;

pub(super) struct Telemetry {
    traces: Vec<NodeTrace>,
    index: HashMap<NodeId, usize>,
    logs: Vec<(NodeId, String)>,
    dropped_logs: usize,
    next_flush: Instant,
    timers: Vec<(NodeId, ticknode_core::TaskId, u32)>,
}

impl Telemetry {
    pub(super) fn new(now: Instant) -> Self {
        Self { traces: Vec::new(), index: HashMap::new(), logs: Vec::new(), dropped_logs: 0, next_flush: now, timers: Vec::new() }
    }

    pub(super) fn add_traces(&mut self, traces: &[NodeTrace]) {
        for trace in traces {
            match self.index.get(&trace.node_id) {
                Some(&slot) => merge(&mut self.traces[slot], trace),
                None => {
                    self.index.insert(trace.node_id, self.traces.len());
                    self.traces.push(trace.clone());
                }
            }
        }
    }

    pub(super) fn add_logs(&mut self, logs: Vec<(NodeId, String)>) {
        let room = MAX_LOGS.saturating_sub(self.logs.len());
        self.dropped_logs += logs.len().saturating_sub(room);
        self.logs.extend(logs.into_iter().take(room));
    }

    /// The messages to send now: empty until the interval has passed or when nothing happened.
    pub(super) fn flush(&mut self, now: Instant, engine: &mut ticknode_core::TickEngine) -> Vec<ServerMessage> {
        if now < self.next_flush {
            return Vec::new();
        }
        self.next_flush = now + INTERVAL;
        let mut messages = Vec::new();
        let (set, cleared) = engine.take_node_error_changes();
        if !set.is_empty() || !cleared.is_empty() {
            messages.push(ServerMessage::NodeErrors { set, cleared });
        }
        let timers = engine.host_state.delay_timers_with_tasks();
        let keys: Vec<_> = timers.iter().map(|(id, timer)| (timer.node, *id, timer.count)).collect();
        if keys != self.timers {
            self.timers = keys;
            messages.push(ServerMessage::Timers(timers.into_iter().map(|(_, timer)| timer).collect()));
        }
        if !self.traces.is_empty() {
            self.index.clear();
            messages.push(ServerMessage::Executed(std::mem::take(&mut self.traces)));
        }
        messages.extend(self.logs.drain(..).map(|(node_id, message)| ServerMessage::Log { node_id, message }));
        if self.dropped_logs > 0 {
            let message = format!("[WARN] {} log lines dropped (more than {MAX_LOGS} in {} ms)", self.dropped_logs, INTERVAL.as_millis());
            messages.push(ServerMessage::Log { node_id: NodeId(ticknode_shared::Uuid::nil()), message });
            self.dropped_logs = 0;
        }
        messages
    }

    /// Nobody listens: drop what was collected so it does not grow without bound.
    pub(super) fn discard(&mut self) {
        self.traces.clear();
        self.index.clear();
        self.logs.clear();
        self.dropped_logs = 0;
    }
}

fn merge(into: &mut NodeTrace, later: &NodeTrace) {
    into.tick = later.tick;
    into.inputs = later.inputs.clone();
    into.outputs = later.outputs.clone();
    for pin in &later.fired {
        if !into.fired.contains(pin) {
            into.fired.push(*pin);
        }
    }
}

#[cfg(test)]
mod tests;
