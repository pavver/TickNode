// Network tests require loopback sockets; run outside restricted sandboxes.
fn plugin_http_engine(runtime: &tokio::runtime::Runtime, url: &str, tick: bool) -> Option<(TickEngine, NodeId)> {
    let bytes = plugin_bytes("test_plugin")?;
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine.add_plugin(&bytes, None, |_| vec![Capability::Http { allowed_domains: vec!["127.0.0.1".into()] }]).unwrap();
    engine
        .enable_plugin(
            "test_plugin",
            &[("token".into(), "good".into()), ("http_url".into(), url.into()), ("http_tick".into(), tick.into())],
        )
        .unwrap();
    let probe = NodeId::generate();
    engine.create_node(probe, "test/http_probe", &[]).unwrap();
    Some((engine, probe))
}

#[test]
fn plugin_http_roundtrip_delivers_configure_and_tick_requests_through_plugin_tick() {
    for tick in [false, true] {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (port, received) = serve_once("plugin reply");
        let Some((mut engine, probe)) = plugin_http_engine(&runtime, &format!("http://127.0.0.1:{port}/poll"), tick)
        else {
            return;
        };
        let done = sink(&mut engine, probe, 4);
        run_until(&mut engine, |e| output(e, probe, 3) == Some(1i64.into()));
        assert_eq!(output(&engine, probe, 1), Some("plugin reply".into()));
        assert!(received.recv_timeout(std::time::Duration::from_secs(1)).unwrap().contains("/poll"));
        engine.tick();
        assert!(engine.executed.iter().any(|t| t.node_id == done));
        assert!(engine.host_state.active_tasks.is_empty(), "plugin HTTP is never delivered to a graph node as a task");
        for _ in 0..3 {
            engine.tick();
        }
        assert_eq!(output(&engine, probe, 3), Some(1i64.into()));
    }
}

#[test]
fn plugin_http_returns_error_status_bodies_without_following_redirects() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for response in [
        "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 7\r\nConnection: close\r\n\r\nlimited",
        "HTTP/1.1 302 Found\r\nLocation: http://forbidden.example/\r\nContent-Length: 7\r\nConnection: close\r\n\r\nlimited",
    ] {
        let (port, _) = serve_raw(response.into());
        let Some((mut engine, probe)) = plugin_http_engine(&runtime, &format!("http://127.0.0.1:{port}/"), false)
        else {
            return;
        };
        run_until(&mut engine, |e| output(e, probe, 3) == Some(1i64.into()));
        assert_eq!(output(&engine, probe, 1), Some("limited".into()));
    }
}

fn pending_http_server() -> (String, std::sync::mpsc::Receiver<()>, std::sync::mpsc::Receiver<bool>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/poll", listener.local_addr().unwrap());
    let (accepted, ready) = std::sync::mpsc::channel();
    let (closed, done) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut headers = Vec::new();
        let mut byte = [0];
        while !headers.ends_with(b"\r\n\r\n") {
            if socket.read(&mut byte).unwrap_or(0) == 0 {
                return;
            }
            headers.push(byte[0]);
        }
        let _ = accepted.send(());
        let _ = closed.send(matches!(socket.read(&mut byte), Ok(0)));
    });
    (url, ready, done)
}

#[test]
fn plugin_http_long_poll_is_aborted_on_disable_crash_and_engine_drop() {
    for action in ["disable", "crash", "drop"] {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (url, accepted, closed) = pending_http_server();
        let Some((mut engine, _)) = plugin_http_engine(&runtime, &url, false) else { return };
        accepted.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
        match action {
            "disable" => {
                engine.disable_plugin("test_plugin").unwrap();
            }
            "crash" => {
                let crash = NodeId::generate();
                engine.create_node(crash, "test/crash", &[]).unwrap();
                engine.trigger_node(crash, PinId(0));
                engine.tick();
            }
            _ => drop(engine),
        }
        assert!(
            closed.recv_timeout(std::time::Duration::from_secs(3)).unwrap(),
            "{action} must close the pending HTTP connection"
        );
    }
}

#[test]
fn plugin_owned_requests_limit_and_cancel_from_configure_without_transport() {
    // This runtime is never driven, so even requests to port 1 cannot reach a socket.
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let Some((mut engine, probe)) = plugin_http_engine(&runtime, "http://127.0.0.1:1/", false) else { return };
    let settings = |cancel| {
        vec![
            ("token".into(), "good".into()),
            ("http_url".into(), "http://127.0.0.1:1/".into()),
            ("http_count".into(), 9i64.into()),
            ("http_cancel".into(), DataValue::Bool(cancel)),
        ]
    };
    engine.configure_plugin("test_plugin", &settings(false)).unwrap();
    engine.tick();
    let status = output(&engine, probe, 2).unwrap();
    assert!(matches!(status, DataValue::String(s) if s.contains("started:8") && s.contains("maximum 8")));
    engine.configure_plugin("test_plugin", &settings(true)).unwrap();
    engine.tick();
    let status = output(&engine, probe, 2).unwrap();
    assert!(matches!(status, DataValue::String(s) if s.contains("cancelled:8")));
    assert_eq!(output(&engine, probe, 3), Some(0i64.into()));
}
