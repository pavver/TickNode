mod actor;
mod plugins;
mod storage;
mod ws;

use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use ticknode_core::{TickBudget, TickEngine};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::storage::Storage;

struct Config {
    bind: String,
    data_dir: PathBuf,
    web_dir: PathBuf,
    bundled_plugins_dir: Option<PathBuf>,
}

impl Config {
    fn from_env() -> Self {
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
        Self {
            bind: var("TICKNODE_BIND", "0.0.0.0:8099"),
            data_dir: var("TICKNODE_DATA_DIR", "data").into(),
            web_dir: var("TICKNODE_WEB_DIR", "ticknode-editor/dist").into(),
            bundled_plugins_dir: std::env::var("TICKNODE_BUNDLED_PLUGINS_DIR").ok().filter(|v| !v.is_empty()).map(PathBuf::from),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let config = Config::from_env();
    std::fs::create_dir_all(&config.data_dir)?;

    let storage = Storage::new(config.data_dir.clone());
    let server_settings = storage.load_settings()?;

    let mut engine = TickEngine::new();
    engine.budget = TickBudget { limit: (server_settings.tick_budget != 0).then_some(server_settings.tick_budget) };
    // Plugins may open connections already in `configure` while loading
    engine.host_state.runtime = Some(tokio::runtime::Handle::current());
    engine.host_state.tick_duration = Duration::from_millis(server_settings.tick_ms as u64);
    // Load plugins before the graph so nodes of their types are instantiated live rather than as stubs
    let mut plugins = plugins::Plugins::new(&config.data_dir).with_bundled_dir(config.bundled_plugins_dir.clone());
    plugins.load_all(&mut engine);

    let loaded = storage.load(&mut engine)?;
    let handle = actor::spawn(engine, loaded, storage, plugins, server_settings)?;

    let app = router(handle.clone(), &config.web_dir);

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    info!(bind = %config.bind, data = %config.data_dir.display(), "TickNode server started");

    let shutdown_handle = handle.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            info!("Stop signal received, saving the graph");
            shutdown_handle.shutdown().await;
            // Open WebSocket connections must not keep the process alive.
            tokio::spawn(async {
                tokio::time::sleep(Duration::from_secs(2)).await;
                std::process::exit(0);
            });
        })
        .await?;
    Ok(())
}

fn router(handle: actor::EngineHandle, web_dir: &Path) -> Router {
    let index = web_dir.join("index.html");
    Router::new()
        .route("/ws", get(ws::handler))
        .fallback_service(ServeDir::new(web_dir).fallback(ServeFile::new(index)))
        .with_state(handle)
}

async fn shutdown_signal() {
    let ctrl_c = async { let _ = tokio::signal::ctrl_c().await; };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => { s.recv().await; }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use ticknode_shared::{ClientMessage, ConnectionModel, DataValue, PinId, Position, ServerMessage, ServerSettings};
    use std::path::PathBuf;
    use tokio_tungstenite::tungstenite::Message;

    type Socket = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

    async fn send(ws: &mut Socket, msg: ClientMessage) {
        ws.send(Message::Binary(postcard::to_allocvec(&msg).unwrap().into())).await.unwrap();
    }

    /// Next graph/command message, skipping telemetry and history availability.
    async fn recv(ws: &mut Socket) -> ServerMessage {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
            if let Message::Binary(b) = msg {
                let msg: ServerMessage = postcard::from_bytes(&b).unwrap();
                if !matches!(msg, ServerMessage::Executed(_) | ServerMessage::Log { .. } | ServerMessage::PropertyOptions { .. } | ServerMessage::History(_)) {
                    return msg;
                }
            }
        }
    }

    /// Next message of any kind (with timeout).
    async fn recv_any(ws: &mut Socket) -> ServerMessage {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
            if let Message::Binary(b) = msg {
                return postcard::from_bytes(&b).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn editor_session_over_websocket_persists_graph() {
        let dir = tempfile::tempdir().unwrap();
        let graph_path = dir.path().join("graph.json");
        let handle = actor::spawn(TickEngine::new(), Default::default(), Storage::new(graph_path.clone()), plugins::Plugins::new(dir.path()), ServerSettings { tick_ms: 10, tick_budget: 0 }).unwrap();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, router(handle.clone(), dir.path())).into_future());

        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr)).await.unwrap();
        let ServerMessage::Init { open_sheet, sheets, .. } = recv(&mut ws).await else { panic!("expected Init") };
        assert_eq!(open_sheet, None, "a new session starts on the home page");
        let sheet_id = sheets[0].info.id;
        send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
        let ServerMessage::Init { definitions, nodes, open_sheet, .. } = recv(&mut ws).await else { panic!("expected Init") };
        assert_eq!(open_sheet, Some(sheet_id));
        assert!(definitions.iter().any(|d| d.type_name == "const"));
        assert!(nodes.is_empty());

        send(&mut ws, ClientMessage::AddNode { type_name: "const".into(), position: Position { x: 1.0, y: 2.0 } }).await;
        let ServerMessage::NodeAdded(c) = recv(&mut ws).await else { panic!() };
        // The initiator learns which node was created after NodeAdded
        assert_eq!(recv(&mut ws).await, ServerMessage::Created(vec![c.model.id]));
        send(&mut ws, ClientMessage::AddNode { type_name: "test/debug".into(), position: Position::default() }).await;
        let ServerMessage::NodeAdded(dbg) = recv(&mut ws).await else { panic!() };
        assert_eq!(recv(&mut ws).await, ServerMessage::Created(vec![dbg.model.id]));

        send(&mut ws, ClientMessage::SetProperty { id: c.model.id, name: "value".into(), value: DataValue::String("hi".into()) }).await;
        let ServerMessage::NodeUpdated(updated) = recv(&mut ws).await else { panic!() };
        assert_eq!(updated.model.properties, vec![("value".to_string(), DataValue::String("hi".into()))]);

        let conn = ConnectionModel { src_node: c.model.id, src_pin: PinId(0), dst_node: dbg.model.id, dst_pin: PinId(1) };
        send(&mut ws, ClientMessage::ConnectData(conn)).await;
        assert_eq!(recv(&mut ws).await, ServerMessage::DataConnected(conn));

        // Invalid connection — error sent only to this client
        send(&mut ws, ClientMessage::ConnectExec(conn)).await;
        assert!(matches!(recv(&mut ws).await, ServerMessage::Error(_)));

        // Node trigger: Debug writes a log, and the server broadcasts the execution trace with inputs
        send(&mut ws, ClientMessage::TriggerNode { id: dbg.model.id, pin: PinId(0) }).await;
        let (mut traced, mut logged) = (false, false);
        while !(traced && logged) {
            match recv_any(&mut ws).await {
                ServerMessage::Executed(traces) => {
                    let t = traces.iter().find(|t| t.node_id == dbg.model.id).unwrap();
                    assert_eq!(t.inputs, vec![(PinId(1), Some(DataValue::String("hi".into())))]);
                    traced = true;
                }
                ServerMessage::Log { node_id, message } => {
                    assert_eq!((node_id, message.as_str()), (dbg.model.id, "hi"));
                    logged = true;
                }
                _ => {}
            }
        }

        // A newly connected editor receives the latest execution traces in its snapshot
        let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr)).await.unwrap();
        let ServerMessage::Init { .. } = recv(&mut ws2).await else { panic!("expected Init") };
        send(&mut ws2, ClientMessage::OpenSheet(Some(sheet_id))).await;
        let ServerMessage::Init { traces, .. } = recv(&mut ws2).await else { panic!("expected Init") };
        assert!(traces.iter().any(|(t, _)| t.node_id == dbg.model.id));

        // Paste a copy of both nodes with their connection: new IDs, preserved positions, Created sent only to initiator and after NodeAdded
        let mut snippet = ticknode_shared::GraphModel {
            nodes: vec![updated.model.clone(), dbg.model.clone()],
            exec_connections: vec![],
            data_connections: vec![conn],
            wire_routes: Vec::new(), detached_connections: Vec::new(), frames: Vec::new(),
        };
        snippet.nodes[0].position = Position { x: 100.0, y: 200.0 };
        send(&mut ws, ClientMessage::Paste(snippet)).await;
        let (mut pasted, mut added, mut connected) = (None, Vec::new(), None);
        while pasted.is_none() || added.len() < 2 || connected.is_none() {
            match recv_any(&mut ws).await {
                ServerMessage::Created(ids) => {
                    assert_eq!(added.len(), 2, "Created before NodeAdded");
                    pasted = Some(ids);
                }
                ServerMessage::NodeAdded(n) => added.push(n),
                ServerMessage::DataConnected(c) => connected = Some(c),
                _ => {}
            }
        }
        let ids = pasted.unwrap();
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&c.model.id) && !ids.contains(&dbg.model.id));
        let copy = added.iter().find(|n| n.model.id == ids[0]).unwrap();
        assert_eq!(copy.model.position, Position { x: 100.0, y: 200.0 });
        assert_eq!(copy.model.properties, vec![("value".to_string(), DataValue::String("hi".into()))]);
        assert_eq!(connected.unwrap(), ConnectionModel { src_node: ids[0], src_pin: PinId(0), dst_node: ids[1], dst_pin: PinId(1) });

        handle.shutdown().await;
        let sheet_file = dir.path().join("sheets").join(format!("{}.json", sheet_id));
        let saved: ticknode_shared::GraphModel = serde_json::from_str(&std::fs::read_to_string(sheet_file).unwrap()).unwrap();
        assert_eq!(saved.nodes.len(), 4);
        assert_eq!(saved.data_connections.len(), 2);
        assert!(saved.data_connections.contains(&conn));
        let saved_const = saved.nodes.iter().find(|n| n.id == c.model.id).unwrap();
        assert_eq!(saved_const.position, Position { x: 1.0, y: 2.0 });
    }

    /// Collects raw bytes of all received messages (to verify that secrets never leak).
    async fn recv_logged(ws: &mut Socket, seen: &mut Vec<u8>) -> ServerMessage {
        loop {
            let msg = tokio::time::timeout(Duration::from_secs(5), ws.next()).await.unwrap().unwrap().unwrap();
            if let Message::Binary(b) = msg {
                seen.extend_from_slice(&b);
                let msg: ServerMessage = postcard::from_bytes(&b).unwrap();
                if !matches!(msg, ServerMessage::Executed(_) | ServerMessage::Log { .. } | ServerMessage::PropertyOptions { .. } | ServerMessage::History(_)) {
                    return msg;
                }
            }
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    struct Server {
        addr: std::net::SocketAddr,
        handle: actor::EngineHandle,
    }

    /// Starts a server on data directory `dir` (same order as `main`: plugins before the graph).
    async fn start(dir: &Path) -> Server {
        let mut engine = TickEngine::new();
        let mut plugins = plugins::Plugins::new(dir);
        plugins.load_all(&mut engine);
        let storage = Storage::new(dir);
        let loaded = storage.load(&mut engine).unwrap();
        let handle = actor::spawn(engine, loaded, storage, plugins, ServerSettings { tick_ms: 10, tick_budget: 0 }).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, router(handle.clone(), dir)).into_future());
        Server { addr, handle }
    }

    fn test_plugin_dir() -> Option<tempfile::TempDir> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
        let Ok(bytes) = std::fs::read(&path) else {
            assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} not found", path.display());
            eprintln!("{} not found - test skipped", path.display());
            return None;
        };
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
        std::fs::write(dir.path().join("plugins/test_plugin.wasm"), bytes).unwrap();
        Some(dir)
    }

    fn plugin_of(plugins: &[ticknode_shared::PluginInfo]) -> &ticknode_shared::PluginInfo {
        plugins.iter().find(|p| p.name == "test_plugin").expect("test_plugin is listed")
    }

    #[tokio::test]
    async fn plugins_are_managed_over_websocket() {
        use ticknode_shared::{PluginState, SettingUpdate, SettingValueView};
        let Some(dir) = test_plugin_dir() else { return };
        let server = start(dir.path()).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr)).await.unwrap();
        let mut seen = Vec::new();

        // Init: plugin is enabled, but configure rejected the empty token
        let ServerMessage::Init { sheets, .. } = recv_logged(&mut ws, &mut seen).await else { panic!("expected Init") };
        let sheet_id = sheets[0].info.id;
        send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
        let ServerMessage::Init { plugins, definitions, translations, .. } = recv_logged(&mut ws, &mut seen).await else { panic!("expected Init") };
        let info = plugin_of(&plugins);
        assert_eq!((info.state, info.file.as_str()), (PluginState::Enabled, "test_plugin.wasm"));
        assert_eq!(info.settings_error.as_ref().map(|e| e.key.as_str()), Some("test-plugin-error-no-token"));
        // Core and plugin translations arrive with Init
        assert!(translations.iter().any(|(l, t)| l == "uk" && t.contains("node-const-title") && t.contains("node-test-greeter-description")));
        assert_eq!(info.settings.len(), 4);
        assert!(info.values.contains(&("token".to_string(), SettingValueView::Secret { is_set: false })));
        assert!(info.values.contains(&("greeting".to_string(), SettingValueView::Value(DataValue::String("Hello".into())))));
        assert!(definitions.iter().any(|d| d.type_name == "test/greeter"));
        assert!(!dir.path().join("plugin-config/test_plugin.json").exists(), "no config file without user actions");

        // Save settings: secret in the response only indicates that it is set
        let secret = "s3cret-token-value";
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![
            ("token".into(), SettingUpdate::Set(DataValue::String(secret.into()))),
            ("greeting".into(), SettingUpdate::Set(DataValue::String("Hello there".into()))),
        ] }).await;
        let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else { panic!("expected PluginUpdated") };
        assert_eq!(info.settings_error, None);
        assert!(info.values.contains(&("token".to_string(), SettingValueView::Secret { is_set: true })));
        assert!(info.values.contains(&("greeting".to_string(), SettingValueView::Value(DataValue::String("Hello there".into())))));

        // Config file: token is saved, permissions are 0600
        let config_path = dir.path().join("plugin-config/test_plugin.json");
        let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(saved["enabled"], true);
        assert!(saved["settings"]["token"].to_string().contains(secret));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&config_path).unwrap().permissions().mode() & 0o777, 0o600);
        }

        // Plugin node sees the updated settings without a restart
        send(&mut ws, ClientMessage::AddNode { type_name: "test/greeter".into(), position: Position { x: 3.0, y: 4.0 } }).await;
        let ServerMessage::NodeAdded(greeter) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!(greeter.unavailable, None);
        assert!(matches!(recv_logged(&mut ws, &mut seen).await, ServerMessage::Created(_)));
        send(&mut ws, ClientMessage::TriggerNode { id: greeter.model.id, pin: PinId(0) }).await;
        loop {
            if let ServerMessage::Executed(traces) = recv_any(&mut ws).await {
                if let Some(t) = traces.iter().find(|t| t.node_id == greeter.model.id) {
                    assert_eq!(t.outputs, vec![(PinId(1), DataValue::String("Hello there|quiet|token".into()))]);
                    break;
                }
            }
        }

        // configure error: plugin stays loaded, error is reported in PluginUpdated
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![("token".into(), SettingUpdate::Set(DataValue::String("bad".into())))] }).await;
        let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!((info.state, info.settings_error.as_ref().map(|e| e.fallback.as_str())), (PluginState::Enabled, Some("Invalid token")));
        // `Keep` preserves the secret; validation error goes only to the initiator
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![
            ("token".into(), SettingUpdate::Set(DataValue::String(secret.into()))),
            ("greeting".into(), SettingUpdate::Set(DataValue::Number(ticknode_shared::NumberValue::Int(1)))),
        ] }).await;
        let ServerMessage::Error(e) = recv_logged(&mut ws, &mut seen).await else { panic!("expected Error") };
        assert_eq!(e.key, "error-setting-invalid");
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![("token".into(), SettingUpdate::Keep)] }).await;
        let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!(info.settings_error.as_ref().map(|e| e.fallback.as_str()), Some("Invalid token"), "Keep does not change the saved token");
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "nope".into(), values: vec![] }).await;
        let ServerMessage::Error(e) = recv_logged(&mut ws, &mut seen).await else { panic!("expected Error") };
        assert_eq!((e.key.as_str(), e.args.clone()), ("error-plugin-not-found", vec![("plugin".to_string(), "nope".to_string())]));

        // Disabling: catalog drops plugin types, node becomes a stub, followed by PluginUpdated
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![("token".into(), SettingUpdate::Set(DataValue::String(secret.into())))] }).await;
        assert!(matches!(recv_logged(&mut ws, &mut seen).await, ServerMessage::PluginUpdated(_)));
        send(&mut ws, ClientMessage::SetPluginEnabled { name: "test_plugin".into(), enabled: false }).await;
        let ServerMessage::Catalog { definitions, types, translations } = recv_logged(&mut ws, &mut seen).await else { panic!("expected Catalog") };
        assert!(!definitions.iter().any(|d| d.type_name == "test/greeter" || d.type_name.starts_with("type/make/")) && types.is_empty());
        assert!(!translations.iter().any(|(_, t)| t.contains("node-test-greeter")), "the disabled plugin takes its translations away");
        let ServerMessage::NodeUpdated(stub) = recv_logged(&mut ws, &mut seen).await else { panic!("expected NodeUpdated") };
        let unavailable = stub.unavailable.as_ref().expect("stub");
        assert_eq!((unavailable.key.as_str(), unavailable.fallback.as_str()), ("error-node-plugin-unavailable", "plugin test_plugin unavailable"));
        assert_eq!(stub.model.type_name, "test/greeter");
        assert_eq!(stub.definition.pins.len(), 2);
        let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!(info.state, PluginState::Disabled);
        let saved: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(saved["enabled"], false);

        // Secret never appeared in any server messages
        assert!(!contains(&seen, secret.as_bytes()), "the secret leaked into messages to the editor");

        // Restart: plugin stays disabled, graph preserves the plugin node as a stub
        server.handle.shutdown().await;
        let sheet_file = dir.path().join("sheets").join(format!("{}.json", sheet_id));
        let on_disk: ticknode_shared::GraphModel = serde_json::from_str(&std::fs::read_to_string(sheet_file).unwrap()).unwrap();
        assert_eq!(on_disk.nodes.len(), 1);
        assert_eq!(on_disk.nodes[0].position, Position { x: 3.0, y: 4.0 });
        let server = start(dir.path()).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr)).await.unwrap();
        let ServerMessage::Init { .. } = recv_logged(&mut ws, &mut seen).await else { panic!() };
        send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
        let ServerMessage::Init { plugins, nodes, .. } = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!(plugin_of(&plugins).state, PluginState::Disabled);
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].unavailable.is_some());

        // Enabling on the fly revives the node with the same settings
        send(&mut ws, ClientMessage::SetPluginEnabled { name: "test_plugin".into(), enabled: true }).await;
        assert!(matches!(recv_logged(&mut ws, &mut seen).await, ServerMessage::Catalog { .. }));
        let ServerMessage::NodeUpdated(live) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!(live.unavailable, None);
        let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else { panic!() };
        assert_eq!((info.state, info.settings_error), (PluginState::Enabled, None));
        assert!(!contains(&seen, secret.as_bytes()));
        server.handle.shutdown().await;
    }

    /// Options published by a plugin reach editors in `Init`, changes are broadcast once, and disabling the
    /// plugin sends the sources again as empty lists.
    #[tokio::test]
    async fn property_options_are_sent_in_init_and_broadcast_on_change() {
        use ticknode_shared::{PropertyOption, SettingUpdate};
        let Some(dir) = test_plugin_dir() else { return };
        let server = start(dir.path()).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr)).await.unwrap();
        let mut seen = Vec::new();

        // Configure (empty token) still publishes the default greeting
        let ServerMessage::Init { property_options, definitions, .. } = recv_logged(&mut ws, &mut seen).await else { panic!("expected Init") };
        let greetings = property_options.iter().find(|s| s.plugin == "test_plugin" && s.source == "greetings").expect("Init carries the options");
        assert_eq!(greetings.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(), ["Hello"]);
        assert!(definitions.iter().any(|d| d.type_name == "test/picker" && !d.properties.is_empty()));

        // Changing the setting republishes the source
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![
            ("greeting".into(), SettingUpdate::Set(DataValue::String("Hola".into()))),
        ] }).await;
        let mut got = None;
        for _ in 0..20 {
            if let ServerMessage::PropertyOptions { plugin, source, options } = recv_any(&mut ws).await {
                got = Some((plugin, source, options));
                break;
            }
        }
        let (plugin, source, options) = got.expect("a PropertyOptions broadcast");
        assert_eq!((plugin.as_str(), source.as_str()), ("test_plugin", "greetings"));
        assert_eq!(options, vec![PropertyOption { value: "Hola".into(), label: "HOLA".into(), detail: "detail Hola".into(), icon: Some("mdi:hand-wave".into()) }]);

        // The same value again does not broadcast anything (only PluginUpdated follows)
        send(&mut ws, ClientMessage::ConfigurePlugin { name: "test_plugin".into(), values: vec![
            ("greeting".into(), SettingUpdate::Set(DataValue::String("Hola".into()))),
        ] }).await;
        loop {
            match recv_any(&mut ws).await {
                ServerMessage::PropertyOptions { .. } => panic!("an unchanged list was broadcast"),
                ServerMessage::PluginUpdated(_) => break,
                _ => {}
            }
        }
        // Give the engine a few ticks to prove nothing follows
        tokio::time::sleep(Duration::from_millis(100)).await;
        while let Ok(Some(Ok(Message::Binary(b)))) = tokio::time::timeout(Duration::from_millis(50), ws.next()).await {
            assert!(!matches!(postcard::from_bytes::<ServerMessage>(&b).unwrap(), ServerMessage::PropertyOptions { .. }), "an unchanged list was broadcast");
        }

        // Disabling removes the sources: they arrive as empty lists
        send(&mut ws, ClientMessage::SetPluginEnabled { name: "test_plugin".into(), enabled: false }).await;
        let mut emptied = Vec::new();
        for _ in 0..40 {
            match recv_any(&mut ws).await {
                // The options follow the plugin events of the same command
                ServerMessage::PropertyOptions { source, options, .. } => {
                    emptied.push((source, options.len()));
                    break;
                }
                _ => {}
            }
        }
        assert!(emptied.contains(&("greetings".to_string(), 0)), "{emptied:?}");

        // A new editor gets nothing for a disabled plugin
        let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr)).await.unwrap();
        let ServerMessage::Init { property_options, .. } = recv_logged(&mut ws2, &mut seen).await else { panic!("expected Init") };
        assert!(property_options.is_empty());
        server.handle.shutdown().await;
    }
}
