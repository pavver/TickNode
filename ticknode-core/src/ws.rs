//! Plugin-owned WebSocket connections.
//!
//! Every plugin instance has one [`WsHub`] (it lives in the instance's `PluginState`), so connections die
//! together with the instance: on disable, after a trap (the instance is recreated and `configure` runs
//! again) and when the engine is dropped. The connection itself is a tokio task on `HostState::runtime`;
//! the plugin side is synchronous: `connect` / `send` / `close` return at once and events are drained
//! once per tick by the engine (`drain`).
//!
//! Only text frames are delivered. Binary frames are ignored (the protocols plugins speak over this are
//! JSON); ping/pong is answered by tungstenite.

use std::collections::HashMap;

use futures::{SinkExt, StreamExt};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue, Request};
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::Message;

pub type WsId = u64;

/// Connections per plugin.
pub const MAX_CONNECTIONS: usize = 4;
/// Largest message accepted in either direction.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Events handed to a plugin per tick; the rest stays queued.
pub const MAX_EVENTS_PER_TICK: usize = 1000;
/// Queued inbound events per plugin; a full queue pauses the connection tasks (backpressure).
const INBOX_CAPACITY: usize = 4096;
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    Opened(WsId),
    Message(WsId, String),
    Closed(WsId, String),
}

struct Connection {
    outbound: mpsc::UnboundedSender<String>,
    task: JoinHandle<()>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub struct WsHub {
    handle: Option<Handle>,
    next_id: WsId,
    connections: HashMap<WsId, Connection>,
    inbox_tx: mpsc::Sender<WsEvent>,
    inbox_rx: mpsc::Receiver<WsEvent>,
}

impl WsHub {
    /// `handle` is the runtime for connection tasks; without it the current runtime is used.
    pub fn new(handle: Option<Handle>) -> Self {
        let (inbox_tx, inbox_rx) = mpsc::channel(INBOX_CAPACITY);
        Self { handle, next_id: 1, connections: HashMap::new(), inbox_tx, inbox_rx }
    }

    /// Sets the runtime for new connections. The instance may be created before the engine gets its runtime
    /// (the server loads plugins first), so the host refreshes it on every call.
    pub fn set_runtime(&mut self, handle: Option<Handle>) {
        if handle.is_some() {
            self.handle = handle;
        }
    }

    /// Starts connecting. URL scheme and headers are validated here; the host check belongs to the caller.
    pub fn connect(&mut self, url: &str, headers: &[(String, String)]) -> Result<WsId, String> {
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err(format!("Too many WebSocket connections (maximum {MAX_CONNECTIONS} per plugin)"));
        }
        let request = build_request(url, headers)?;
        let handle = self.handle.clone()
            .or_else(|| Handle::try_current().ok())
            .ok_or("WebSocket unavailable: the engine is running without a tokio runtime")?;
        let id = self.next_id;
        self.next_id += 1;
        let (outbound, outbound_rx) = mpsc::unbounded_channel();
        let task = handle.spawn(run(id, request, outbound_rx, self.inbox_tx.clone()));
        self.connections.insert(id, Connection { outbound, task });
        Ok(id)
    }

    pub fn send(&mut self, id: WsId, text: String) -> Result<(), String> {
        if text.len() > MAX_MESSAGE_BYTES {
            return Err(format!("Message is too large ({} bytes, maximum {MAX_MESSAGE_BYTES})", text.len()));
        }
        let conn = self.connections.get(&id).ok_or_else(|| format!("Unknown or closed WebSocket connection {id}"))?;
        conn.outbound.send(text).map_err(|_| format!("WebSocket connection {id} is closed"))
    }

    /// Closes the connection. Events it still has queued are discarded by `drain`.
    pub fn close(&mut self, id: WsId) {
        self.connections.remove(&id);
    }

    /// Takes up to `max` queued events in arrival order. Events of connections closed by the plugin are
    /// dropped; a `Closed` event forgets its connection (freeing a slot).
    pub fn drain(&mut self, max: usize) -> Vec<WsEvent> {
        let mut events = Vec::new();
        while events.len() < max {
            let Ok(event) = self.inbox_rx.try_recv() else { break };
            let id = match &event {
                WsEvent::Opened(id) | WsEvent::Message(id, _) | WsEvent::Closed(id, _) => *id,
            };
            if !self.connections.contains_key(&id) {
                continue;
            }
            if matches!(event, WsEvent::Closed(..)) {
                self.connections.remove(&id);
            }
            events.push(event);
        }
        events
    }
}

fn build_request(url: &str, headers: &[(String, String)]) -> Result<Request<()>, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
    if !matches!(parsed.scheme(), "ws" | "wss") {
        return Err(format!("Unsupported scheme: {} (expected ws or wss)", parsed.scheme()));
    }
    let mut request = url.into_client_request().map_err(|e| format!("Invalid WebSocket request: {e}"))?;
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|e| format!("Invalid header name {name:?}: {e}"))?;
        let value = HeaderValue::from_str(value).map_err(|e| format!("Invalid value of header {name}: {e}"))?;
        request.headers_mut().insert(name, value);
    }
    Ok(request)
}

async fn run(id: WsId, request: Request<()>, mut outbound: mpsc::UnboundedReceiver<String>, inbox: mpsc::Sender<WsEvent>) {
    let reason = connection(id, request, &mut outbound, &inbox).await;
    let _ = inbox.send(WsEvent::Closed(id, reason)).await;
}

/// Runs the connection until it ends; returns the reason.
async fn connection(id: WsId, request: Request<()>, outbound: &mut mpsc::UnboundedReceiver<String>, inbox: &mpsc::Sender<WsEvent>) -> String {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let connecting = tokio_tungstenite::connect_async_with_config(request, Some(config), false);
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Ok(Ok((stream, _))) => stream,
        Ok(Err(e)) => return format!("Connection failed: {e}"),
        Err(_) => return "Connection failed: timed out".to_string(),
    };
    if inbox.send(WsEvent::Opened(id)).await.is_err() {
        return "Host shut down".to_string();
    }
    let (mut sink, mut source) = stream.split();
    loop {
        tokio::select! {
            frame = source.next() => match frame {
                Some(Ok(Message::Text(text))) => {
                    if inbox.send(WsEvent::Message(id, text.to_string())).await.is_err() {
                        return "Host shut down".to_string();
                    }
                }
                // tungstenite queues the pong; flushing sends it even if the plugin is silent
                Some(Ok(Message::Ping(_))) => { let _ = sink.flush().await; }
                Some(Ok(Message::Close(frame))) => {
                    return frame.map_or("Closed by the server".to_string(), |f| format!("Closed by the server: {} {}", u16::from(f.code), f.reason));
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => return format!("Connection error: {e}"),
                None => return "Connection closed".to_string(),
            },
            text = outbound.recv() => match text {
                Some(text) => if let Err(e) = sink.send(Message::Text(text.into())).await {
                    return format!("Send failed: {e}");
                },
                None => return "Closed by the plugin".to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_validation() {
        assert!(build_request("ws://127.0.0.1:1/x", &[("x-a".into(), "b".into())]).is_ok());
        assert!(build_request("wss://example.com/ws", &[]).is_ok());
        assert!(build_request("http://example.com", &[]).is_err());
        assert!(build_request("not a url", &[]).is_err());
        assert!(build_request("ws://example.com", &[("bad name".into(), "b".into())]).is_err());
    }

    #[test]
    fn connection_limit_and_unknown_ids() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let mut hub = WsHub::new(Some(rt.handle().clone()));
        for _ in 0..MAX_CONNECTIONS {
            hub.connect("ws://127.0.0.1:1/", &[]).unwrap();
        }
        assert!(hub.connect("ws://127.0.0.1:1/", &[]).unwrap_err().contains("Too many"));
        assert!(hub.send(999, "x".into()).is_err());
        hub.close(1);
        assert!(hub.connect("ws://127.0.0.1:1/", &[]).is_ok());
    }
}
