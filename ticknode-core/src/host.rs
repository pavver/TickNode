//! Engine services for nodes: asynchronous tasks (delays, HTTP), logs, and current tick number.
//! Used by both built-in nodes and WASM plugins (via `plugin.rs`).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use ticknode_shared::{DataValue, NodeId, NodeTimer, SheetId, UiMessage, VariableKey};

pub type TaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutput {
    Delay,
    Http(HttpResponse),
}

pub type TaskResult = Result<TaskOutput, String>;

pub const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_HTTP_BODY: u64 = 4 * 1024 * 1024;

enum TaskState {
    Delay { start_tick: u64, due_tick: u64 },
    Spawned(tokio::task::JoinHandle<TaskResult>),
}

pub struct HostTask {
    pub node_id: NodeId,
    state: TaskState,
}

pub struct HostState {
    pub locale: String,
    pub display: ticknode_shared::DisplayRules,
    /// Pending tasks. Completed tasks are removed by the engine and delivered to the node via `Node::on_task`.
    pub active_tasks: HashMap<TaskId, HostTask>,
    next_task_id: TaskId,
    pub current_tick: u64,
    pub tick_duration: Duration,
    /// Runtime for network tasks. When `None`, the current runtime (`Handle::try_current`) is used.
    pub runtime: Option<tokio::runtime::Handle>,
    /// Node log messages (Console, Debug, plugins) for the current tick; collected by the server after the tick.
    pub logs: Vec<(NodeId, String)>,
    pub(crate) errors: Vec<(NodeId, UiMessage, std::time::Instant)>,
    /// Keys of variables created or set to a different value during the previous tick.
    pub changed_variables: HashSet<VariableKey>,
    /// Live sheet variables currently in the store, for shadowed `Auto` lookups.
    pub active_sheet_variables: HashSet<(SheetId, String)>,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            display: Default::default(),
            active_tasks: HashMap::new(),
            next_task_id: 0,
            current_tick: 0,
            tick_duration: Duration::from_millis(100),
            runtime: None,
            logs: Vec::new(),
            errors: Vec::new(),
            changed_variables: HashSet::new(),
            active_sheet_variables: HashSet::new(),
        }
    }
}

impl HostState {
    pub fn log(&mut self, node_id: NodeId, message: impl Into<String>) {
        self.logs.push((node_id, message.into()));
    }

    /// Keeps the existing English log while also reporting a structured node error.
    pub fn error(&mut self, node: NodeId, message: UiMessage, log: String) {
        self.log(node, log);
        self.report_error(node, message);
    }

    /// Reports a continuing failure independently of node-specific log throttling.
    pub(crate) fn report_error(&mut self, node: NodeId, message: UiMessage) {
        if !node.0.is_nil() {
            self.errors.push((node, message, std::time::Instant::now()));
        }
    }

    /// Number of ticks needed to cover `duration` (rounded up).
    pub fn ticks_for(&self, duration: Duration) -> u64 {
        let tick = self.tick_duration.max(Duration::from_millis(1)).as_nanos();
        duration.as_nanos().div_ceil(tick).min(u128::from(u64::MAX)) as u64
    }

    /// Tick durations pass through; wall-clock durations round up to whole engine ticks.
    pub fn ticks_for_value(&self, value: &DataValue) -> Option<u64> {
        match value {
            DataValue::Ticks(n) => Some(*n),
            DataValue::Duration(d) => Some(self.ticks_for(*d)),
            _ => None,
        }
    }

    fn insert(&mut self, node_id: NodeId, state: TaskState) -> TaskId {
        let id = self.next_task_id;
        self.next_task_id += 1;
        self.active_tasks.insert(id, HostTask { node_id, state });
        id
    }

    /// The task will complete at tick `current_tick + ticks` (no earlier than the next tick).
    pub fn start_delay(&mut self, node_id: NodeId, ticks: u64) -> TaskId {
        let due_tick = self.current_tick.saturating_add(ticks.max(1));
        self.insert(node_id, TaskState::Delay { start_tick: self.current_tick, due_tick })
    }

    /// One timer per node and its first task ID, for telemetry change detection.
    /// Sort by node to keep the result stable despite the task map's iteration order.
    pub fn delay_timers_with_tasks(&self) -> Vec<(TaskId, NodeTimer)> {
        let mut first = std::collections::BTreeMap::new();
        for (&id, task) in &self.active_tasks {
            let TaskState::Delay { start_tick, due_tick } = task.state else { continue };
            let entry = first.entry(task.node_id).or_insert((start_tick, id, due_tick, 0u32));
            entry.3 = entry.3.saturating_add(1);
            if (start_tick, id) < (entry.0, entry.1) {
                *entry = (start_tick, id, due_tick, entry.3);
            }
        }
        let ms = self.tick_duration.as_millis().min(u128::from(u64::MAX)) as u64;
        first.into_iter().map(|(node, (start, id, due, count))| (id, NodeTimer {
            node,
            total_ms: due.saturating_sub(start).saturating_mul(ms),
            remaining_ms: due.saturating_sub(self.current_tick).saturating_mul(ms),
            count,
        })).collect()
    }

    /// Spawns an HTTP request in the background. Domain permissions are checked by the caller (plugin host).
    pub fn start_http(&mut self, node_id: NodeId, request: HttpRequest) -> Result<TaskId, String> {
        let runtime = self.runtime.clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok())
            .ok_or("HTTP unavailable: the engine is running without a tokio runtime")?;
        let handle = runtime.spawn_blocking(move || perform_http_request(request));
        Ok(self.insert(node_id, TaskState::Spawned(handle)))
    }

    /// Cancels a task if it belongs to `node_id`.
    pub fn cancel_task(&mut self, node_id: NodeId, id: TaskId) -> bool {
        if self.active_tasks.get(&id).is_some_and(|t| t.node_id == node_id) {
            if let Some(task) = self.active_tasks.remove(&id) {
                abort(task);
            }
            true
        } else {
            false
        }
    }

    pub fn cancel_node_tasks(&mut self, node_id: NodeId) {
        let ids: Vec<_> = self.active_tasks.iter().filter(|(_, t)| t.node_id == node_id).map(|(id, _)| *id).collect();
        for id in ids {
            self.cancel_task(node_id, id);
        }
    }

    /// Drains tasks completed up to and including the current tick, in start order.
    pub(crate) fn take_finished(&mut self) -> Vec<(NodeId, TaskId, TaskResult)> {
        let tick = self.current_tick;
        let mut ready: Vec<TaskId> = self.active_tasks.iter()
            .filter(|(_, t)| match &t.state {
                TaskState::Delay { due_tick, .. } => *due_tick <= tick,
                TaskState::Spawned(handle) => handle.is_finished(),
            })
            .map(|(id, _)| *id)
            .collect();
        ready.sort_unstable();
        ready.into_iter()
            .filter_map(|id| self.active_tasks.remove(&id).map(|t| (id, t)))
            .map(|(id, task)| {
                let result = match task.state {
                    TaskState::Delay { .. } => Ok(TaskOutput::Delay),
                    // The task has already finished, so block_on does not wait.
                    TaskState::Spawned(handle) => futures::executor::block_on(handle)
                        .unwrap_or_else(|e| Err(format!("Task terminated abnormally: {}", e))),
                };
                (task.node_id, id, result)
            })
            .collect()
    }
}

fn abort(task: HostTask) {
    if let TaskState::Spawned(handle) = task.state {
        handle.abort();
    }
}

fn perform_http_request(request: HttpRequest) -> TaskResult {
    let timeout = if request.timeout.is_zero() { DEFAULT_HTTP_TIMEOUT } else { request.timeout.min(MAX_HTTP_TIMEOUT) };
    // No automatic redirects: allowed domains are checked only for the requested URL.
    // A 3xx response is returned to the node; to follow `Location`, it must make a new request (which is checked).
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut req = agent.request(&request.method, &request.url).timeout(timeout);
    for (name, value) in &request.headers {
        req = req.set(name, value);
    }
    let response = match req.send_bytes(&request.body) {
        Ok(r) => r,
        // A 4xx/5xx status code is still a valid response that the node can handle itself.
        Err(ureq::Error::Status(_, r)) => r,
        Err(e) => return Err(e.to_string()),
    };
    let status = response.status();
    let headers = response.headers_names().into_iter()
        .filter_map(|name| response.header(&name).map(|v| (name.clone(), v.to_string())))
        .collect();
    let mut body = Vec::new();
    use std::io::Read;
    response.into_reader().take(MAX_HTTP_BODY).read_to_end(&mut body).map_err(|e| e.to_string())?;
    Ok(TaskOutput::Http(HttpResponse { status, headers, body }))
}

#[cfg(test)]
mod tests;
