use crate::*;

pub type TaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 0 — default host timeout.
    pub timeout_ms: u32,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self { method: "GET".into(), url: url.into(), headers: Vec::new(), body: Vec::new(), timeout_ms: 0 }
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self { method: "POST".into(), body: body.into(), ..Self::get(url) }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutput {
    Delay,
    Http(HttpResponse),
}

/// Reason why a node is executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Pulse on an input exec pin (or manual trigger from the editor).
    Pin(PinId),
    /// A task started by this node has completed.
    Task { id: TaskId, result: Result<TaskOutput, String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Pulse from an output exec pin.
    Exec(PinId),
    /// Value on an output data pin.
    Data(PinId, DataValue),
    /// Remove the current value from an output data pin.
    Clear(PinId),
}

/// Handle of a plugin-owned WebSocket connection (see [`host::ws_connect`]). Void after the plugin
/// instance is recreated following a crash.
pub type WsId = u64;

/// Plugin event delivered to [`Plugin::tick`]. The historical name also includes HTTP completions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    /// The connection is established.
    Opened(WsId),
    /// A text frame was received (binary frames are never delivered).
    Message(WsId, String),
    /// The connection ended or could not be opened; carries the reason.
    Closed(WsId, String),
    /// A task started without a current node finished; delivered only to its owning instance.
    HttpDone { id: TaskId, result: Result<TaskOutput, String> },
}

/// Events delivered to the plugin tick (WebSocket events and plugin-owned HTTP completions).
pub type PluginEvent = WsEvent;

/// Outputs for one node produced outside of its own `execute`/`poll`: applied by the host as if the
/// node had executed (exec pulses queue downstream nodes for the next tick, data goes to the node's outputs).
/// Emissions for foreign or unknown nodes and for invalid pins or values are dropped with an error log.
#[derive(Debug, Clone, PartialEq)]
pub struct Emission {
    pub node: NodeId,
    pub outputs: Vec<Output>,
}

impl Emission {
    pub fn new(node: NodeId, outputs: Vec<Output>) -> Self {
        Self { node, outputs }
    }
}
