use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use ticknode_shared::*;
use tracing::warn;
use wasmtime::component::ResourceTable;
use wasmtime::StoreLimits;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use crate::host::{HostState, HttpRequest, TaskId};
use crate::ws::WsHub;
use super::{bindings, lock, wit, OptionStore};
use super::options::sanitize_options;

/// Capabilities granted to the plugin at load time.
#[derive(Default)]
pub(super) struct Grants {
    pub(super) name: String,
    pub(super) http_domains: Vec<String>,
    pub(super) ws_hosts: Vec<String>,
    pub(super) storage_dir: Option<PathBuf>,
}

/// Currently executing node: owns spawned tasks and log messages.
pub(super) struct CallCtx {
    pub(super) node_id: NodeId,
    pub(super) host: HostState,
}

pub(super) struct PluginState {
    pub(super) wasi: WasiCtx,
    pub(super) table: ResourceTable,
    pub(super) limits: StoreLimits,
    pub(super) grants: Arc<Grants>,
    pub(super) call: Option<CallCtx>,
    /// Plugin-owned WebSocket connections; they die with this instance.
    pub(super) ws: WsHub,
    pub(super) options: Arc<Mutex<OptionStore>>,
}

impl WasiView for PluginState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl PluginState {
    fn call(&mut self) -> Result<&mut CallCtx, String> {
        self.call.as_mut().ok_or_else(|| "Only available during execute/poll".to_string())
    }
}

impl wit::Host for PluginState {}

impl bindings::ticknode::plugin::host::Host for PluginState {
    fn log(&mut self, level: wit::LogLevel, message: String) {
        let error = matches!(level, wit::LogLevel::Error).then(|| UiMessage::plain(message.clone()));
        let message = match level {
            wit::LogLevel::Debug | wit::LogLevel::Info => message,
            wit::LogLevel::Warn => format!("[WARN] {message}"),
            wit::LogLevel::Error => format!("[ERROR] {message}"),
        };
        match &mut self.call {
            Some(c) => match error {
                Some(error) => c.host.error(c.node_id, error, message),
                None => c.host.log(c.node_id, message),
            },
            None => warn!(plugin = %self.grants.name, "{}", message),
        }
    }

    fn current_tick(&mut self) -> u64 {
        self.call.as_ref().map_or(0, |c| c.host.current_tick)
    }

    fn tick_duration_ms(&mut self) -> u32 {
        self.call.as_ref().map_or(0, |c| c.host.tick_duration.as_millis() as u32)
    }

    fn delay(&mut self, ticks: u64) -> Result<TaskId, String> {
        let c = self.call()?;
        Ok(c.host.start_delay(c.node_id, ticks))
    }

    fn start_http(&mut self, request: wit::HttpRequest) -> Result<TaskId, String> {
        let url = url::Url::parse(&request.url).map_err(|e| format!("Invalid URL: {e}"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(format!("Unsupported scheme: {}", url.scheme()));
        }
        let host = url.host_str().unwrap_or_default();
        if !self.grants.http_domains.iter().any(|d| domain_allowed(host, d)) {
            return Err(format!("The plugin is not allowed to make HTTP requests to {host}"));
        }
        let request = HttpRequest {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout: std::time::Duration::from_millis(request.timeout_ms as u64),
        };
        let c = self.call()?;
        c.host.start_http(c.node_id, request)
    }

    fn cancel_task(&mut self, id: TaskId) -> bool {
        self.call().is_ok_and(|c| c.host.cancel_task(c.node_id, id))
    }

    fn ws_connect(&mut self, url: String, headers: Vec<(String, String)>) -> Result<wit::WsId, String> {
        let parsed = url::Url::parse(&url).map_err(|e| format!("Invalid URL: {e}"))?;
        if !matches!(parsed.scheme(), "ws" | "wss") {
            return Err(format!("Unsupported scheme: {} (expected ws or wss)", parsed.scheme()));
        }
        let host = parsed.host_str().unwrap_or_default();
        if !self.grants.ws_hosts.iter().any(|d| domain_allowed(host, d)) {
            return Err(format!("The plugin is not allowed to open WebSocket connections to {host}"));
        }
        self.ws.connect(&url, &headers)
    }

    fn ws_send(&mut self, id: wit::WsId, text: String) -> Result<(), String> {
        self.ws.send(id, text)
    }

    fn ws_close(&mut self, id: wit::WsId) {
        self.ws.close(id);
    }

    fn set_options(&mut self, source: String, options: Vec<wit::PropertyOption>) {
        let (accepted, warnings) = {
            let store = lock(&self.options);
            let source_count_ok = store.sources.contains_key(&source) || store.sources.len() < MAX_OPTION_SOURCES;
            sanitize_options(source_count_ok, &source, options)
        };
        for w in warnings {
            self.log(wit::LogLevel::Warn, w);
        }
        lock(&self.options).set(&source, accepted);
    }
}

/// `example.com` allows the domain itself and its subdomains, but not `evilexample.com`.
pub(super) fn domain_allowed(host: &str, allowed: &str) -> bool {
    allowed == "*" || host == allowed || host.ends_with(&format!(".{}", allowed))
}
