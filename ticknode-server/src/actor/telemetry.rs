//! Execution telemetry for editors, sent at a fixed rate instead of after every tick.
//!
//! Ticks may be as short as 1 ms; one `Executed` message per tick would flood the editors (and the
//! broadcast channel) without showing anything a human can see. Traces of one interval are merged per
//! node (latest inputs/outputs, union of fired pins), logs are kept in order up to a limit.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use ticknode_shared::{
    NodeError, NodeId, NodeTimer, NodeTrace, ServerMessage, SheetId, Variable, VariableKey, VariableScope,
};

/// How often accumulated telemetry is sent to editors.
pub(super) const INTERVAL: Duration = Duration::from_millis(100);
/// Log lines sent per interval; the rest are counted and reported as one line.
pub(super) const MAX_LOGS: usize = 1000;

pub(crate) struct Telemetry {
    traces: Vec<NodeTrace>,
    index: HashMap<NodeId, usize>,
    logs: Vec<(NodeId, String)>,
    dropped_logs: usize,
    next_flush: Instant,
    sheet_timers: HashMap<Option<SheetId>, Vec<(NodeId, ticknode_core::TaskId, u32)>>,
}

impl Telemetry {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            traces: Vec::new(),
            index: HashMap::new(),
            logs: Vec::new(),
            dropped_logs: 0,
            next_flush: now,
            sheet_timers: HashMap::new(),
        }
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
    pub(super) fn flush(
        &mut self,
        now: Instant,
        engine: &mut ticknode_core::TickEngine,
        node_sheet: &HashMap<NodeId, SheetId>,
    ) -> Vec<(Option<SheetId>, ServerMessage)> {
        if now < self.next_flush {
            return Vec::new();
        }
        self.next_flush = now + INTERVAL;
        let mut messages = Vec::new();

        // Node errors split per sheet
        let (set, cleared) = engine.take_node_error_changes();
        if !set.is_empty() || !cleared.is_empty() {
            let mut sheet_errors: HashMap<Option<SheetId>, (Vec<NodeError>, Vec<NodeId>)> = HashMap::new();
            for err in set {
                let sheet = node_sheet.get(&err.node).copied();
                sheet_errors.entry(sheet).or_default().0.push(err);
            }
            for id in cleared {
                let sheet = node_sheet.get(&id).copied();
                sheet_errors.entry(sheet).or_default().1.push(id);
            }
            for (sheet, (sheet_set, sheet_cleared)) in sheet_errors {
                messages.push((sheet, ServerMessage::NodeErrors { set: sheet_set, cleared: sheet_cleared }));
            }
        }

        // Timers split per sheet
        let timers = engine.host_state.delay_timers_with_tasks();
        let mut current_sheet_timers: HashMap<Option<SheetId>, (Vec<(NodeId, ticknode_core::TaskId, u32)>, Vec<NodeTimer>)> = HashMap::new();
        for (task_id, timer) in timers {
            let sheet = node_sheet.get(&timer.node).copied();
            let entry = current_sheet_timers.entry(sheet).or_default();
            entry.0.push((timer.node, task_id, timer.count));
            entry.1.push(timer);
        }

        let all_sheets: BTreeSet<Option<SheetId>> = self.sheet_timers.keys().copied()
            .chain(current_sheet_timers.keys().copied())
            .collect();

        for sheet in all_sheets {
            let (new_keys, timer_list) = current_sheet_timers.remove(&sheet).unwrap_or_default();
            let changed = match self.sheet_timers.get(&sheet) {
                Some(old_keys) => old_keys != &new_keys,
                None => !new_keys.is_empty(),
            };
            if changed {
                if new_keys.is_empty() {
                    self.sheet_timers.remove(&sheet);
                } else {
                    self.sheet_timers.insert(sheet, new_keys);
                }
                messages.push((sheet, ServerMessage::Timers(timer_list)));
            }
        }

        // Traces split per sheet
        if !self.traces.is_empty() {
            self.index.clear();
            let traces = std::mem::take(&mut self.traces);
            let mut sheet_traces: HashMap<Option<SheetId>, Vec<NodeTrace>> = HashMap::new();
            for trace in traces {
                let sheet = node_sheet.get(&trace.node_id).copied();
                sheet_traces.entry(sheet).or_default().push(trace);
            }
            for (sheet, list) in sheet_traces {
                messages.push((sheet, ServerMessage::Executed(list)));
            }
        }

        // Variable changes split per sheet
        let changes = engine.variables.take_changes();
        if !changes.set.is_empty() || !changes.removed.is_empty() {
            let mut sheet_changes: HashMap<Option<SheetId>, (Vec<Variable>, Vec<VariableKey>)> = HashMap::new();
            for var in changes.set {
                let sheet = match var.key.scope {
                    VariableScope::Sheet => var.key.sheet,
                    VariableScope::Global | VariableScope::Persistent => None,
                };
                sheet_changes.entry(sheet).or_default().0.push(var);
            }
            for key in changes.removed {
                let sheet = match key.scope {
                    VariableScope::Sheet => key.sheet,
                    VariableScope::Global | VariableScope::Persistent => None,
                };
                sheet_changes.entry(sheet).or_default().1.push(key);
            }
            for (sheet, (set, removed)) in sheet_changes {
                messages.push((sheet, ServerMessage::VariablesChanged { set, removed }));
            }
        }

        // Logs are global
        messages.extend(self.logs.drain(..).map(|(node_id, message)| (None, ServerMessage::Log { node_id, message })));
        if self.dropped_logs > 0 {
            let message = format!("[WARN] {} log lines dropped (more than {MAX_LOGS} in {} ms)", self.dropped_logs, INTERVAL.as_millis());
            messages.push((None, ServerMessage::Log { node_id: NodeId(ticknode_shared::Uuid::nil()), message }));
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
