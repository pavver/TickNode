// --- Plugin-level WebSocket connections and emissions ---

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Local WebSocket echo server (replies `echo:<text>`). It runs in its own runtime on its own thread:
/// the engine under test must stay outside any tokio task.
struct EchoServer {
    port: u16,
    opened: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
    /// Value of the `x-test` request header of the last connection.
    header: Arc<std::sync::Mutex<Option<String>>>,
}

fn echo_server() -> EchoServer {
    use futures::{SinkExt, StreamExt};
    let server = EchoServer {
        port: 0,
        opened: Arc::default(),
        closed: Arc::default(),
        header: Arc::default(),
    };
    let (opened, closed, header) = (
        server.opened.clone(),
        server.closed.clone(),
        server.header.clone(),
    );
    let (port_tx, port_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_tx.send(listener.local_addr().unwrap().port()).unwrap();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let (opened, closed, header) = (opened.clone(), closed.clone(), header.clone());
                tokio::spawn(async move {
                    let callback =
                        |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
                            *header.lock().unwrap() = req
                                .headers()
                                .get("x-test")
                                .and_then(|v| v.to_str().ok())
                                .map(String::from);
                            Ok(resp)
                        };
                    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await
                    else {
                        return;
                    };
                    opened.fetch_add(1, Ordering::SeqCst);
                    while let Some(Ok(msg)) = ws.next().await {
                        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                            let _ = ws.send(format!("echo:{text}").into()).await;
                        }
                    }
                    closed.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
    });
    EchoServer {
        port: port_rx.recv().unwrap(),
        ..server
    }
}

fn ws_settings(url: &str) -> Vec<(String, DataValue)> {
    let mut settings = test_settings("Hi", "good");
    settings.push(("ws_url".into(), DataValue::String(url.into())));
    settings
}

fn engine_with_ws(
    url: &str,
    hosts: Vec<String>,
    runtime: &tokio::runtime::Runtime,
) -> Option<TickEngine> {
    let bytes = plugin_bytes("test_plugin")?;
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine
        .add_plugin(&bytes, None, |_| {
            vec![Capability::WebSocket {
                allowed_hosts: hosts,
            }]
        })
        .unwrap();
    engine
        .enable_plugin("test_plugin", &ws_settings(url))
        .unwrap();
    Some(engine)
}

fn status(engine: &TickEngine, node: NodeId) -> Option<DataValue> {
    output(engine, node, 4)
}

#[test]
fn websocket_roundtrip_reaches_node_outputs_through_plugin_tick() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(
        &format!("ws://127.0.0.1:{}/", server.port),
        vec!["127.0.0.1".into()],
        &runtime,
    ) else {
        return;
    };
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);

    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    constant(&mut engine, DataValue::String("hello".into()), probe, 1);
    let received = sink(&mut engine, probe, 2);

    // The plugin opened the connection in `configure`; `opened` arrives in plugin-tick and is
    // pushed to the node without any execute.
    run_until(&mut engine, |e| {
        status(e, probe) == Some(DataValue::String("open".into()))
    });
    assert_eq!(
        server.header.lock().unwrap().as_deref(),
        Some("1"),
        "custom headers reach the server"
    );

    engine.trigger_node(probe, PinId(0));
    run_until(&mut engine, |e| output(e, probe, 3).is_some());
    assert_eq!(
        output(&engine, probe, 3),
        Some(DataValue::String("echo:hello".into()))
    );
    // The exec pulse from the emission queues the downstream node, and the trace records it
    run_until(&mut engine, |e| {
        e.execution_queue.contains(&(received, PinId(0)))
            || e.executed.iter().any(|t| t.node_id == received)
    });
}

#[test]
fn websocket_works_when_the_runtime_is_set_after_the_plugin_is_loaded() {
    // The server loads plugins before the engine thread gets its runtime: the existing instance must
    // pick the runtime up later instead of failing with "running without a tokio runtime".
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(bytes) = plugin_bytes("test_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine
        .add_plugin(&bytes, None, |_| {
            vec![Capability::WebSocket {
                allowed_hosts: vec!["*".into()],
            }]
        })
        .unwrap();
    engine
        .enable_plugin("test_plugin", &ws_settings(""))
        .unwrap();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine
        .configure_plugin(
            "test_plugin",
            &ws_settings(&format!("ws://127.0.0.1:{}/", server.port)),
        )
        .unwrap();
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    run_until(&mut engine, |e| {
        status(e, probe) == Some(DataValue::String("open".into()))
    });
}

#[test]
fn websocket_emission_trace_is_recorded() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(
        &format!("ws://127.0.0.1:{}/", server.port),
        vec!["*".into()],
        &runtime,
    ) else {
        return;
    };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    let mut traced = false;
    for _ in 0..200 {
        engine.tick();
        traced |= engine
            .executed
            .iter()
            .any(|t| t.node_id == probe && t.outputs.iter().any(|(p, _)| p.0 == 4));
        if traced {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(traced, "the emission is recorded like an execution");
}

#[test]
fn websocket_to_a_host_outside_the_capability_is_denied() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    // Only example.com is allowed: connecting to 127.0.0.1 fails in `configure`, and nothing connects
    let Some(engine) = engine_with_ws(
        &format!("ws://127.0.0.1:{}/", server.port),
        vec!["example.com".into()],
        &runtime,
    ) else {
        return;
    };
    let error = engine
        .plugin_settings_error("test_plugin")
        .expect("configure reports the denial");
    assert!(error.fallback.contains("not allowed"), "{error:?}");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(server.opened.load(Ordering::SeqCst), 0);

    // Wrong scheme is rejected as well
    let Some(mut engine) = engine_with_ws("", vec!["*".into()], &runtime) else {
        return;
    };
    let err = engine
        .configure_plugin("test_plugin", &ws_settings("http://127.0.0.1:1/"))
        .unwrap_err();
    assert!(err.fallback.contains("Unsupported scheme"), "{err:?}");
}

#[test]
fn websocket_connection_closes_when_the_plugin_is_disabled() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(
        &format!("ws://127.0.0.1:{}/", server.port),
        vec!["*".into()],
        &runtime,
    ) else {
        return;
    };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    run_until(&mut engine, |e| {
        status(e, probe) == Some(DataValue::String("open".into()))
    });
    assert_eq!(server.closed.load(Ordering::SeqCst), 0);

    engine.disable_plugin("test_plugin").unwrap();
    for _ in 0..100 {
        if server.closed.load(Ordering::SeqCst) == 1 {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("the server did not observe the connection closing");
}

#[test]
fn websocket_reconnects_after_the_plugin_crashes() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(
        &format!("ws://127.0.0.1:{}/", server.port),
        vec!["*".into()],
        &runtime,
    ) else {
        return;
    };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    run_until(&mut engine, |e| {
        status(e, probe) == Some(DataValue::String("open".into()))
    });

    engine.trigger_node(crash, PinId(0));
    // The old connection is closed with the dead instance; configure runs again on the new one and reconnects
    run_until(&mut engine, |_| {
        server.opened.load(Ordering::SeqCst) == 2 && server.closed.load(Ordering::SeqCst) >= 1
    });
    // The recreated probe node (same graph id) receives the new connection's events
    constant(&mut engine, DataValue::String("again".into()), probe, 1);
    engine.trigger_node(probe, PinId(0));
    run_until(&mut engine, |e| {
        output(e, probe, 3) == Some(DataValue::String("echo:again".into()))
    });
}

