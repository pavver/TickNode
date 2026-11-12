use super::*;
use futures::{SinkExt, StreamExt};
use std::path::PathBuf;
pub(super) use ticknode_shared::*;
use tokio_tungstenite::tungstenite::Message;

pub(super) type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub(super) async fn send(ws: &mut Socket, msg: ClientMessage) {
    ws.send(Message::Binary(postcard::to_allocvec(&msg).unwrap().into()))
        .await
        .unwrap();
}

/// Next graph/command message, skipping telemetry and history availability.
pub(super) async fn recv(ws: &mut Socket) -> ServerMessage {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Binary(b) = msg {
            let msg: ServerMessage = postcard::from_bytes(&b).unwrap();
            if !matches!(
                msg,
                ServerMessage::Executed(_)
                    | ServerMessage::Log { .. }
                    | ServerMessage::PropertyOptions { .. }
                    | ServerMessage::History(_)
            ) {
                return msg;
            }
        }
    }
}

/// Next message of any kind (with timeout).
pub(super) async fn recv_any(ws: &mut Socket) -> ServerMessage {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Binary(b) = msg {
            return postcard::from_bytes(&b).unwrap();
        }
    }
}

/// Collects raw bytes of all received messages (to verify that secrets never leak).
pub(super) async fn recv_logged(ws: &mut Socket, seen: &mut Vec<u8>) -> ServerMessage {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Binary(b) = msg {
            seen.extend_from_slice(&b);
            let msg: ServerMessage = postcard::from_bytes(&b).unwrap();
            if !matches!(
                msg,
                ServerMessage::Executed(_)
                    | ServerMessage::Log { .. }
                    | ServerMessage::PropertyOptions { .. }
                    | ServerMessage::History(_)
            ) {
                return msg;
            }
        }
    }
}

pub(super) fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

pub(super) struct Server {
    pub(super) addr: std::net::SocketAddr,
    pub(super) handle: actor::EngineHandle,
}

/// Starts a server on data directory `dir` (same order as `main`: plugins before the graph).
pub(super) async fn start(dir: &Path) -> Server {
    let mut engine = TickEngine::new();
    let mut plugins = plugins::Plugins::new(dir);
    plugins.load_all(&mut engine);
    let storage = Storage::new(dir);
    let loaded = storage.load(&mut engine).unwrap();
    let handle = actor::spawn(
        engine,
        loaded,
        storage,
        plugins,
        ServerSettings {
            tick_ms: 10,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router(handle.clone(), dir)).into_future());
    Server { addr, handle }
}

pub(super) fn test_plugin_dir() -> Option<tempfile::TempDir> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(&path) else {
        assert!(
            std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(),
            "{} not found",
            path.display()
        );
        eprintln!("{} not found - test skipped", path.display());
        return None;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
    std::fs::write(dir.path().join("plugins/test_plugin.wasm"), bytes).unwrap();
    Some(dir)
}

pub(super) fn plugin_of(plugins: &[ticknode_shared::PluginInfo]) -> &ticknode_shared::PluginInfo {
    plugins
        .iter()
        .find(|p| p.name == "test_plugin")
        .expect("test_plugin is listed")
}

mod session;
mod settings;
mod telemetry;
