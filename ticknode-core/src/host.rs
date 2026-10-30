//! Engine services for nodes: asynchronous tasks (delays, HTTP), logs, and current tick number.
//! Used by both built-in nodes and WASM plugins (via `plugin.rs`).

use std::collections::HashMap;
use std::time::Duration;

use ticknode_shared::NodeId;

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
    Delay { due_tick: u64 },
    Spawned(tokio::task::JoinHandle<TaskResult>),
}

pub struct HostTask {
    pub node_id: NodeId,
    state: TaskState,
}

pub struct HostState {
    /// Pending tasks. Completed tasks are removed by the engine and delivered to the node via `Node::on_task`.
    pub active_tasks: HashMap<TaskId, HostTask>,
    next_task_id: TaskId,
    pub current_tick: u64,
    pub tick_duration: Duration,
    /// Runtime for network tasks. When `None`, the current runtime (`Handle::try_current`) is used.
    pub runtime: Option<tokio::runtime::Handle>,
    /// Node log messages (Console, Debug, plugins) for the current tick; collected by the server after the tick.
    pub logs: Vec<(NodeId, String)>,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            active_tasks: HashMap::new(),
            next_task_id: 0,
            current_tick: 0,
            tick_duration: Duration::from_millis(100),
            runtime: None,
            logs: Vec::new(),
        }
    }
}

impl HostState {
    pub fn log(&mut self, node_id: NodeId, message: impl Into<String>) {
        self.logs.push((node_id, message.into()));
    }

    /// Number of ticks needed to cover `duration` (rounded up).
    pub fn ticks_for(&self, duration: Duration) -> u64 {
        let tick = self.tick_duration.as_millis().max(1);
        duration.as_millis().div_ceil(tick) as u64
    }

    fn insert(&mut self, node_id: NodeId, state: TaskState) -> TaskId {
        let id = self.next_task_id;
        self.next_task_id += 1;
        self.active_tasks.insert(id, HostTask { node_id, state });
        id
    }

    /// The task will complete at tick `current_tick + ticks` (no earlier than the next tick).
    pub fn start_delay(&mut self, node_id: NodeId, ticks: u64) -> TaskId {
        let due_tick = self.current_tick + ticks.max(1);
        self.insert(node_id, TaskState::Delay { due_tick })
    }

    /// Spawns an HTTP request in the background. Domain permissions are checked by the caller (plugin host).
    pub fn start_http(&mut self, node_id: NodeId, request: HttpRequest) -> Result<TaskId, String> {
        let runtime = self.runtime.clone()
            .or_else(|| tokio::runtime::Handle::try_current().ok())
            .ok_or("HTTP недоступний: рушій запущено без tokio runtime")?;
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
                TaskState::Delay { due_tick } => *due_tick <= tick,
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
                        .unwrap_or_else(|e| Err(format!("Задача аварійно завершилась: {}", e))),
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
