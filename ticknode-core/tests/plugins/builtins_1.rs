#[test]
fn manifest_and_definitions_are_registered() {
    let Some(bytes) = plugin_bytes("math_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    let manifest = engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert_eq!(manifest.name, "math_plugin");
    let pow = engine.registry.get_definition("math/pow").unwrap();
    let names: Vec<_> = pow.pins.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["In", "Base", "Exp", "Out", "Res"]);

    // Reloading cannot hijack already registered types.
    assert!(engine.load_plugin(&bytes, None, |_| vec![]).is_err());
}

#[test]
fn math_plugin_computes() {
    let Some(mut engine) = engine_with("math_plugin", vec![]) else {
        return;
    };
    let pow = NodeId::generate();
    engine.create_node(pow, "math/pow", &[]).unwrap();
    constant(&mut engine, number(2), pow, 32);
    constant(&mut engine, number(10), pow, 33);
    let next = sink(&mut engine, pow, 31);

    engine.trigger_node(pow, PinId(30));
    engine.tick();
    assert_eq!(output(&engine, pow, 34), Some(number(1024)));
    assert!(engine.execution_queue.contains(&(next, PinId(0))));
}

#[test]
fn string_plugin_regex_split_aggregate() {
    let Some(mut engine) = engine_with("string_plugin", vec![]) else {
        return;
    };

    let regex = NodeId::generate();
    engine.create_node(regex, "string/regex", &[]).unwrap();
    constant(
        &mut engine,
        DataValue::String("hello 123 world".into()),
        regex,
        52,
    );
    constant(&mut engine, DataValue::String(r"(\d+)".into()), regex, 53);
    engine.trigger_node(regex, PinId(50));
    engine.tick();
    assert_eq!(output(&engine, regex, 54), Some(DataValue::Bool(true)));
    assert_eq!(
        output(&engine, regex, 55),
        Some(DataValue::String("123".into()))
    );

    let split = NodeId::generate();
    engine.create_node(split, "string/split", &[]).unwrap();
    constant(
        &mut engine,
        DataValue::String("apple,banana,cherry".into()),
        split,
        42,
    );
    constant(&mut engine, DataValue::String(",".into()), split, 43);
    let aggregate = NodeId::generate();
    engine
        .create_node(aggregate, "string/aggregate", &[])
        .unwrap();
    engine
        .connect_data(split, PinId(44), aggregate, PinId(62))
        .unwrap();
    constant(&mut engine, DataValue::String(" - ".into()), aggregate, 63);

    engine.trigger_node(split, PinId(40));
    engine.tick();
    engine.trigger_node(aggregate, PinId(60));
    engine.tick();
    assert_eq!(
        output(&engine, aggregate, 64),
        Some(DataValue::String("apple - banana - cherry".into()))
    );
}

#[test]
fn delay_queue_fires_after_delay() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else {
        return;
    };
    let delay = NodeId::generate();
    engine.create_node(delay, "delay/queue", &[]).unwrap();
    // 500 ms at 100 ms per tick is 5 ticks.
    constant(
        &mut engine,
        DataValue::Duration(std::time::Duration::from_millis(500)),
        delay,
        2,
    );
    let next = sink(&mut engine, delay, 1);

    engine.trigger_node(delay, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);
    for _ in 0..4 {
        engine.tick();
        assert!(!engine.execution_queue.contains(&(next, PinId(0))));
    }
    engine.tick();
    assert!(engine.execution_queue.contains(&(next, PinId(0))));
    assert!(engine.host_state.active_tasks.is_empty());
}

#[test]
fn delay_nodes_keep_their_own_state() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else {
        return;
    };
    let ignore = NodeId::generate();
    engine.create_node(ignore, "delay/ignore", &[]).unwrap();
    constant(&mut engine, DataValue::Ticks(3), ignore, 12);
    let reset = NodeId::generate();
    engine.create_node(reset, "delay/reset", &[]).unwrap();
    constant(&mut engine, DataValue::Ticks(3), reset, 22);

    // Repeated impulses: Ignore keeps a single task, Reset replaces its own.
    for _ in 0..3 {
        engine.trigger_node(ignore, PinId(10));
        engine.trigger_node(reset, PinId(20));
        engine.tick();
    }
    let owners: Vec<_> = engine
        .host_state
        .active_tasks
        .values()
        .map(|t| t.node_id)
        .collect();
    assert_eq!(owners.iter().filter(|n| **n == ignore).count(), 1);
    assert_eq!(owners.iter().filter(|n| **n == reset).count(), 1);
    let timer = |engine: &TickEngine, node| {
        engine
            .delay_timers()
            .into_iter()
            .find(|t| t.node == node)
            .unwrap()
    };
    assert_eq!(
        (
            timer(&engine, ignore).total_ms,
            timer(&engine, ignore).remaining_ms
        ),
        (300, 100)
    );
    assert_eq!(
        (
            timer(&engine, reset).total_ms,
            timer(&engine, reset).remaining_ms
        ),
        (300, 300)
    );
    engine.tick();
    assert_eq!(engine.delay_timers().len(), 1);
    assert_eq!(engine.delay_timers()[0].node, reset);
    engine.tick();
    engine.tick();
    assert!(engine.delay_timers().is_empty());
}

#[test]
fn removing_plugin_node_cancels_tasks() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else {
        return;
    };
    let delay = NodeId::generate();
    engine.create_node(delay, "delay/queue", &[]).unwrap();
    constant(
        &mut engine,
        DataValue::Duration(std::time::Duration::from_secs(1)),
        delay,
        2,
    );
    engine.trigger_node(delay, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);
    engine.remove_node(delay);
    assert!(engine.host_state.active_tasks.is_empty());
}

/// One-shot HTTP server responding with `body`. The receiver gets the request line (`GET /… HTTP/1.1`).
fn serve_once(body: &'static str) -> (u16, std::sync::mpsc::Receiver<String>) {
    serve_raw(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    ))
}

fn serve_raw(response: String) -> (u16, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        let _ = tx.send(request.lines().next().unwrap_or_default().to_string());
        stream.write_all(response.as_bytes()).unwrap();
    });
    (port, rx)
}

fn run_until(engine: &mut TickEngine, mut done: impl FnMut(&TickEngine) -> bool) {
    for _ in 0..200 {
        engine.tick();
        if done(engine) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the condition was not met within 200 ticks");
}

#[test]
fn weather_plugin_makes_http_request() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let Some(mut engine) = engine_with(
        "http_weather_plugin",
        vec![Capability::Http {
            allowed_domains: vec!["127.0.0.1".into()],
        }],
    ) else {
        return;
    };
    engine.host_state.runtime = Some(runtime.handle().clone());

    let (port, requests) = serve_once("25.4,Rainy");
    let weather = NodeId::generate();
    let url = format!("http://127.0.0.1:{port}/weather?city={{city}}");
    engine
        .create_node(
            weather,
            "http/weather",
            &[("url".into(), DataValue::String(url.clone()))],
        )
        .unwrap();
    assert_eq!(
        engine.node_model(weather).unwrap().properties,
        vec![("url".into(), DataValue::String(url))]
    );
    // City is URL-encoded and does not inject extra query parameters
    constant(
        &mut engine,
        DataValue::String("Lviv&units=imperial".into()),
        weather,
        1,
    );
    let success = sink(&mut engine, weather, 2);

    engine.trigger_node(weather, PinId(0));
    run_until(&mut engine, |e| {
        e.execution_queue.contains(&(success, PinId(0)))
    });
    assert_eq!(
        output(&engine, weather, 4),
        Some(DataValue::Number(NumberValue::Float(25.4)))
    );
    assert_eq!(
        output(&engine, weather, 5),
        Some(DataValue::String("Rainy".into()))
    );
    assert_eq!(
        requests.recv().unwrap(),
        "GET /weather?city=Lviv%26units%3Dimperial HTTP/1.1"
    );
}

#[test]
fn http_redirects_are_not_followed() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let Some(mut engine) = engine_with(
        "http_weather_plugin",
        vec![Capability::Http {
            allowed_domains: vec!["127.0.0.1".into()],
        }],
    ) else {
        return;
    };
    engine.host_state.runtime = Some(runtime.handle().clone());

    // Redirect to an unpermitted host (localhost) — the node receives 3xx rather than the target response
    let (target, target_requests) = serve_once("1,Hacked");
    let (port, _) = serve_raw(format!(
        "HTTP/1.1 302 Found\r\nLocation: http://localhost:{target}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    ));
    let weather = NodeId::generate();
    engine
        .create_node(
            weather,
            "http/weather",
            &[(
                "url".into(),
                DataValue::String(format!("http://127.0.0.1:{port}/{{city}}")),
            )],
        )
        .unwrap();
    let failure = sink(&mut engine, weather, 3);

    engine.trigger_node(weather, PinId(0));
    run_until(&mut engine, |e| {
        e.execution_queue.contains(&(failure, PinId(0)))
    });
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == weather && m.contains("HTTP 302")),
        "{:?}",
        engine.host_state.logs
    );
    assert!(
        target_requests.try_recv().is_err(),
        "the host followed the redirect"
    );
}

#[test]
fn http_outside_granted_domains_is_denied() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Only example.com is allowed — a request to 127.0.0.1 is rejected immediately.
    let Some(mut engine) = engine_with(
        "http_weather_plugin",
        vec![Capability::Http {
            allowed_domains: vec!["example.com".into()],
        }],
    ) else {
        return;
    };
    engine.host_state.runtime = Some(runtime.handle().clone());

    let weather = NodeId::generate();
    engine
        .create_node(
            weather,
            "http/weather",
            &[(
                "url".into(),
                DataValue::String("http://127.0.0.1:9/{city}".into()),
            )],
        )
        .unwrap();
    let failure = sink(&mut engine, weather, 3);
    engine.trigger_node(weather, PinId(0));
    engine.tick();

    assert!(engine.host_state.active_tasks.is_empty());
    assert!(engine.execution_queue.contains(&(failure, PinId(0))));
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == weather && m.contains("not allowed"))
    );
}

fn crash_setup() -> Option<(TickEngine, NodeId, NodeId)> {
    let mut engine = engine_with("test_plugin", vec![])?;
    let counter = NodeId::generate();
    engine
        .create_node(counter, "test/counter", &[("step".into(), number(5))])
        .unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    Some((engine, counter, crash))
}

fn assert_recovers(trigger_pin: u32, reason: &str) {
    let Some((mut engine, counter, crash)) = crash_setup() else {
        return;
    };
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(5)));

    // A crash does not bring down the engine: the error is logged to the node's console.
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == crash && m.contains("Plugin crashed") && m.contains(reason)),
        "{:?}",
        engine.host_state.logs
    );
    assert!(
        engine
            .node_errors()
            .iter()
            .any(|error| error.node == crash && error.message.key == "error-plugin-node-crashed")
    );

    // Instance recreated: counter is reconstructed from properties (step 5) with fresh state.
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(5)));

    // The offending node remains halted until reconfigured.
    engine.host_state.logs.clear();
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == crash && m.contains("stopped"))
    );
}

#[test]
fn removing_node_after_crash_does_not_touch_new_instance() {
    let Some((mut engine, counter, crash)) = crash_setup() else {
        return;
    };
    // Both nodes hold resources in the first instance
    engine.trigger_node(counter, PinId(0));
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    engine.tick();

    // Instance is dead; removing a node must neither create a new instance nor pass it a stale resource
    engine.remove_node(crash);
    engine.remove_node(counter);
    let counter = NodeId::generate();
    engine.create_node(counter, "test/counter", &[]).unwrap();
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(1)));
}
