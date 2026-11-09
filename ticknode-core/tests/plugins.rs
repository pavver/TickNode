//! Integration tests for WASM plugins. Requires built components:
//! `cargo build --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p test-plugin -p ha-plugin`.
//! If a binary is missing, the test is skipped with a message (or fails when `TICKNODE_REQUIRE_PLUGINS=1`).
//!
//! Tests are intentionally non-async: synchronous wasmtime-wasi cannot be called inside a tokio task,
//! so the engine, as on the server, runs outside the runtime while a runtime handle is passed for HTTP.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;

use ticknode_core::*;
use ticknode_shared::*;

fn plugin_bytes(name: &str) -> Option<Vec<u8>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../target/wasm32-wasip2/debug/{name}.wasm"));
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(_) if std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none() => {
            eprintln!("{} not found - test skipped", path.display());
            None
        }
        Err(e) => panic!("{}: {e}", path.display()),
    }
}

fn engine_with(plugin: &str, capabilities: Vec<Capability>) -> Option<TickEngine> {
    let bytes = plugin_bytes(plugin)?;
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| capabilities).unwrap();
    Some(engine)
}

fn constant(engine: &mut TickEngine, value: DataValue, dst: NodeId, dst_pin: u32) {
    let id = NodeId::generate();
    engine.create_node(id, "const", &[("value".into(), value)]).unwrap();
    engine.connect_data(id, PinId(0), dst, PinId(dst_pin)).unwrap();
}

fn sink(engine: &mut TickEngine, src: NodeId, src_pin: u32) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, "test/debug", &[]).unwrap();
    engine.connect_exec(src, PinId(src_pin), id, PinId(0)).unwrap();
    id
}

fn number(v: i64) -> DataValue {
    DataValue::Number(NumberValue::Int(v))
}

fn output(engine: &TickEngine, node: NodeId, pin: u32) -> Option<DataValue> {
    engine.data_cache.get(&node)?.get(&PinId(pin)).cloned()
}

#[test]
fn manifest_and_definitions_are_registered() {
    let Some(bytes) = plugin_bytes("math_plugin") else { return };
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
    let Some(mut engine) = engine_with("math_plugin", vec![]) else { return };
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
    let Some(mut engine) = engine_with("string_plugin", vec![]) else { return };

    let regex = NodeId::generate();
    engine.create_node(regex, "string/regex", &[]).unwrap();
    constant(&mut engine, DataValue::String("hello 123 world".into()), regex, 52);
    constant(&mut engine, DataValue::String(r"(\d+)".into()), regex, 53);
    engine.trigger_node(regex, PinId(50));
    engine.tick();
    assert_eq!(output(&engine, regex, 54), Some(DataValue::Bool(true)));
    assert_eq!(output(&engine, regex, 55), Some(DataValue::String("123".into())));

    let split = NodeId::generate();
    engine.create_node(split, "string/split", &[]).unwrap();
    constant(&mut engine, DataValue::String("apple,banana,cherry".into()), split, 42);
    constant(&mut engine, DataValue::String(",".into()), split, 43);
    let aggregate = NodeId::generate();
    engine.create_node(aggregate, "string/aggregate", &[]).unwrap();
    engine.connect_data(split, PinId(44), aggregate, PinId(62)).unwrap();
    constant(&mut engine, DataValue::String(" - ".into()), aggregate, 63);

    engine.trigger_node(split, PinId(40));
    engine.tick();
    engine.trigger_node(aggregate, PinId(60));
    engine.tick();
    assert_eq!(output(&engine, aggregate, 64), Some(DataValue::String("apple - banana - cherry".into())));
}

#[test]
fn delay_queue_fires_after_delay() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else { return };
    let delay = NodeId::generate();
    engine.create_node(delay, "delay/queue", &[]).unwrap();
    // 500 ms at 100 ms per tick is 5 ticks.
    constant(&mut engine, DataValue::Duration(std::time::Duration::from_millis(500)), delay, 2);
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
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else { return };
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
    let owners: Vec<_> = engine.host_state.active_tasks.values().map(|t| t.node_id).collect();
    assert_eq!(owners.iter().filter(|n| **n == ignore).count(), 1);
    assert_eq!(owners.iter().filter(|n| **n == reset).count(), 1);
    let timer = |engine: &TickEngine, node| engine.delay_timers().into_iter().find(|t| t.node == node).unwrap();
    assert_eq!((timer(&engine, ignore).total_ms, timer(&engine, ignore).remaining_ms), (300, 100));
    assert_eq!((timer(&engine, reset).total_ms, timer(&engine, reset).remaining_ms), (300, 300));
    engine.tick();
    assert_eq!(engine.delay_timers().len(), 1);
    assert_eq!(engine.delay_timers()[0].node, reset);
    engine.tick();
    engine.tick();
    assert!(engine.delay_timers().is_empty());
}

#[test]
fn removing_plugin_node_cancels_tasks() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else { return };
    let delay = NodeId::generate();
    engine.create_node(delay, "delay/queue", &[]).unwrap();
    constant(&mut engine, DataValue::Duration(std::time::Duration::from_secs(1)), delay, 2);
    engine.trigger_node(delay, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);
    engine.remove_node(delay);
    assert!(engine.host_state.active_tasks.is_empty());
}

/// One-shot HTTP server responding with `body`. The receiver gets the request line (`GET /… HTTP/1.1`).
fn serve_once(body: &'static str) -> (u16, std::sync::mpsc::Receiver<String>) {
    serve_raw(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body))
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
    let Some(mut engine) = engine_with("http_weather_plugin", vec![Capability::Http { allowed_domains: vec!["127.0.0.1".into()] }]) else { return };
    engine.host_state.runtime = Some(runtime.handle().clone());

    let (port, requests) = serve_once("25.4,Rainy");
    let weather = NodeId::generate();
    let url = format!("http://127.0.0.1:{port}/weather?city={{city}}");
    engine.create_node(weather, "http/weather", &[("url".into(), DataValue::String(url.clone()))]).unwrap();
    assert_eq!(engine.node_model(weather).unwrap().properties, vec![("url".into(), DataValue::String(url))]);
    // City is URL-encoded and does not inject extra query parameters
    constant(&mut engine, DataValue::String("Lviv&units=imperial".into()), weather, 1);
    let success = sink(&mut engine, weather, 2);

    engine.trigger_node(weather, PinId(0));
    run_until(&mut engine, |e| e.execution_queue.contains(&(success, PinId(0))));
    assert_eq!(output(&engine, weather, 4), Some(DataValue::Number(NumberValue::Float(25.4))));
    assert_eq!(output(&engine, weather, 5), Some(DataValue::String("Rainy".into())));
    assert_eq!(requests.recv().unwrap(), "GET /weather?city=Lviv%26units%3Dimperial HTTP/1.1");
}

#[test]
fn http_redirects_are_not_followed() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let Some(mut engine) = engine_with("http_weather_plugin", vec![Capability::Http { allowed_domains: vec!["127.0.0.1".into()] }]) else { return };
    engine.host_state.runtime = Some(runtime.handle().clone());

    // Redirect to an unpermitted host (localhost) — the node receives 3xx rather than the target response
    let (target, target_requests) = serve_once("1,Hacked");
    let (port, _) = serve_raw(format!("HTTP/1.1 302 Found\r\nLocation: http://localhost:{target}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
    let weather = NodeId::generate();
    engine.create_node(weather, "http/weather", &[("url".into(), DataValue::String(format!("http://127.0.0.1:{port}/{{city}}")))]).unwrap();
    let failure = sink(&mut engine, weather, 3);

    engine.trigger_node(weather, PinId(0));
    run_until(&mut engine, |e| e.execution_queue.contains(&(failure, PinId(0))));
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == weather && m.contains("HTTP 302")), "{:?}", engine.host_state.logs);
    assert!(target_requests.try_recv().is_err(), "the host followed the redirect");
}

#[test]
fn http_outside_granted_domains_is_denied() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Only example.com is allowed — a request to 127.0.0.1 is rejected immediately.
    let Some(mut engine) = engine_with("http_weather_plugin", vec![Capability::Http { allowed_domains: vec!["example.com".into()] }]) else { return };
    engine.host_state.runtime = Some(runtime.handle().clone());

    let weather = NodeId::generate();
    engine.create_node(weather, "http/weather", &[("url".into(), DataValue::String("http://127.0.0.1:9/{city}".into()))]).unwrap();
    let failure = sink(&mut engine, weather, 3);
    engine.trigger_node(weather, PinId(0));
    engine.tick();

    assert!(engine.host_state.active_tasks.is_empty());
    assert!(engine.execution_queue.contains(&(failure, PinId(0))));
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == weather && m.contains("not allowed")));
}

fn crash_setup() -> Option<(TickEngine, NodeId, NodeId)> {
    let mut engine = engine_with("test_plugin", vec![])?;
    let counter = NodeId::generate();
    engine.create_node(counter, "test/counter", &[("step".into(), number(5))]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    Some((engine, counter, crash))
}

fn assert_recovers(trigger_pin: u32, reason: &str) {
    let Some((mut engine, counter, crash)) = crash_setup() else { return };
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(5)));

    // A crash does not bring down the engine: the error is logged to the node's console.
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("Plugin crashed") && m.contains(reason)), "{:?}", engine.host_state.logs);

    // Instance recreated: counter is reconstructed from properties (step 5) with fresh state.
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(5)));

    // The offending node remains halted until reconfigured.
    engine.host_state.logs.clear();
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("stopped")));
}

#[test]
fn removing_node_after_crash_does_not_touch_new_instance() {
    let Some((mut engine, counter, crash)) = crash_setup() else { return };
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

#[test]
fn only_polling_plugin_nodes_are_polled() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let ticker = NodeId::generate();
    engine.create_node(ticker, "test/ticker", &[]).unwrap();
    let counter = NodeId::generate();
    engine.create_node(counter, "test/counter", &[]).unwrap();
    for _ in 0..3 {
        engine.tick();
    }
    // Ticker's `poll` counts calls; counter is not polled (its `poll` panics)
    assert_eq!(output(&engine, ticker, 1), Some(number(3)));
    assert!(engine.host_state.logs.is_empty(), "{:?}", engine.host_state.logs);
}

#[test]
fn plugin_panic_restarts_instance() {
    assert_recovers(0, "panic");
}

#[test]
fn infinite_loop_is_stopped_by_fuel() {
    assert_recovers(1, "compute limit");
}

#[test]
fn storage_is_isolated_plugin_directory() {
    let Some(bytes) = plugin_bytes("test_plugin") else { return };
    let root = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, Some(root.path()), |m| m.requested_capabilities.clone()).unwrap();

    let storage = NodeId::generate();
    engine.create_node(storage, "test/storage", &[]).unwrap();
    constant(&mut engine, DataValue::String("hello".into()), storage, 1);
    engine.trigger_node(storage, PinId(0));
    engine.tick();

    assert_eq!(output(&engine, storage, 2), Some(DataValue::String("hello".into())));
    let file = root.path().join("test_plugin/note.txt");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "hello");
}

#[test]
fn storage_without_grant_is_unavailable() {
    let Some(bytes) = plugin_bytes("test_plugin") else { return };
    let root = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, Some(root.path()), |_| vec![]).unwrap();

    let storage = NodeId::generate();
    engine.create_node(storage, "test/storage", &[]).unwrap();
    engine.trigger_node(storage, PinId(0));
    engine.tick();

    let Some(DataValue::String(result)) = output(&engine, storage, 2) else { panic!("no result") };
    assert!(result.starts_with("error:"), "{result}");
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

// --- Custom data types ---

fn point_type() -> DataType {
    DataType::Named("test.point".into())
}

fn mode_type() -> DataType {
    DataType::Named("test.mode".into())
}

#[test]
fn plugin_types_are_registered_with_make_and_break_nodes() {
    let Some(engine) = engine_with("test_plugin", vec![]) else { return };
    let point = engine.types.get("test.point").expect("the type is registered");
    assert!(point.is_struct());
    assert_eq!(point.color.as_deref(), Some("#e0a030"));
    assert_eq!(engine.types.get("test.mode").unwrap().values().len(), 2);

    let make = engine.registry.get_definition("type/make/test.point").unwrap();
    assert_eq!(make.title, "Make Point");
    assert_eq!(make.category, "Data");
    assert!(!make.help.is_empty());
    assert!(make.pins.iter().any(|p| p.id == PinId(0) && p.direction == PinDirection::Output && p.pin_type == PinType::Data(point_type())));
    assert_eq!(make.pins.iter().filter(|p| p.direction == PinDirection::Input).count(), 3);
    let brk = engine.registry.get_definition("type/break/test.point").unwrap();
    assert!(!brk.help.is_empty());
    assert!(brk.pins.iter().any(|p| p.id == PinId(0) && p.direction == PinDirection::Input));
    // Make/Break nodes are not generated for enums
    assert!(engine.registry.get_definition("type/make/test.mode").is_none());
}

#[test]
fn type_conflicts_and_unknown_types_are_rejected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let before = engine.registry.get_definitions().len();

    let dup = TypeDefinition::new_enum("test.mode", "Other", &[("a", "A")]);
    let err = engine.register_types(&[dup], &[]).unwrap_err();
    assert!(err.contains("test.mode"), "{err}");

    let unknown = TypeDefinition::new_struct("other.thing", "Thing", vec![FieldDefinition::new("p", "P", DataType::Named("nope.type".into()))]);
    let err = engine.register_types(&[unknown], &[]).unwrap_err();
    assert!(err.contains("nope.type"), "{err}");

    let bad_name = TypeDefinition::new_enum("nodot", "X", &[("a", "A")]);
    assert!(engine.register_types(&[bad_name], &[]).is_err());

    // Node pin referencing an unknown type
    let node = NodeDefinition {
        adaptive: false,
        cost: 1,
        properties: Vec::new(),
        type_name: "other/node".into(),
        title: "N".into(),
        category: "Test".into(),
        description: String::new(),
        help: String::new(),
        pins: vec![PinDefinition { id: PinId(0), name: "P".into(), pin_type: PinType::Data(DataType::Named("nope.type".into())), direction: PinDirection::Input, required: false }],
    };
    let err = engine.register_types(&[], &[node]).unwrap_err();
    assert!(err.contains("nope.type"), "{err}");

    // A field may reference a previously registered type
    let nested = TypeDefinition::new_struct("other.nested", "Nested", vec![FieldDefinition::new("p", "P", point_type())]);
    engine.register_types(&[nested], &[]).unwrap();
    assert!(engine.registry.get_definition("type/make/other.nested").is_some());
    assert_eq!(engine.registry.get_definitions().len(), before + 2);

    // Failed attempt left no partial state behind
    assert!(engine.types.get("other.thing").is_none());
}

#[test]
fn pin_compatibility_respects_named_types() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let brk = NodeId::generate();
    engine.create_node(brk, "type/break/test.point", &[]).unwrap();
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    let get_field = NodeId::generate();
    engine.create_node(get_field, "object/get_field", &[]).unwrap();
    let object = NodeId::generate();
    engine.create_node(object, "object/create", &[]).unwrap();
    let debug = NodeId::generate();
    engine.create_node(debug, "test/debug", &[]).unwrap();

    // Named -> Named (same), Named -> Any, Named -> Object
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine.connect_data(make, PinId(0), debug, PinId(1)).unwrap();
    engine.connect_data(make, PinId(0), get_field, PinId(0)).unwrap();
    // Object -> Named is rejected (strict typing)
    assert!(matches!(engine.connect_data(object, PinId(0), brk, PinId(0)), Err(EngineError::TypeMismatch(..))));
    assert!(matches!(engine.connect_data(object, PinId(0), sink, PinId(1)), Err(EngineError::TypeMismatch(..))));
    // Different Named type and String -> Enum
    assert!(engine.connect_data(make, PinId(0), sink, PinId(2)).is_err());
    let text = NodeId::generate();
    engine.create_node(text, "const", &[("value".into(), DataValue::String("auto".into()))]).unwrap();
    engine.connect_data(text, PinId(0), sink, PinId(2)).unwrap();
    // Number -> Named
    let num = NodeId::generate();
    engine.create_node(num, "const", &[("value".into(), number(1))]).unwrap();
    assert!(engine.connect_data(num, PinId(0), sink, PinId(1)).is_err());
}

#[test]
fn enum_pin_to_string_is_allowed() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    // Break has no enum output, so test the rule directly through the type registry
    assert!(engine.types.compatible(&mode_type(), &DataType::String));
    assert!(!engine.types.compatible(&DataType::String, &mode_type()));
    assert!(engine.types.compatible(&point_type(), &DataType::Object));
    assert!(!engine.types.compatible(&DataType::Object, &point_type()));
    assert!(engine.types.compatible(&DataType::List(Box::new(point_type())), &DataType::List(Box::new(DataType::Any))));
    assert!(!engine.types.compatible(&mode_type(), &point_type()));
    let _ = &mut engine;
}

#[test]
fn make_and_break_roundtrip_through_plugin_nodes() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let source = NodeId::generate();
    engine.create_node(source, "test/point_source", &[("x".into(), number(5)), ("y".into(), number(7))]).unwrap();
    let brk = NodeId::generate();
    engine.create_node(brk, "type/break/test.point", &[]).unwrap();
    engine.connect_data(source, PinId(1), brk, PinId(0)).unwrap();

    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    // y is an inline value on the node; label is optional but provided
    engine.set_input_value(make, PinId(2), Some(number(1))).unwrap();
    engine.set_input_value(make, PinId(3), Some(DataValue::String("hi".into()))).unwrap();

    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine.set_input_value(sink, PinId(2), Some(DataValue::String("manual".into()))).unwrap();

    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 3), Some(DataValue::String("manual:5,1:hi".into())));
    assert_eq!(output(&engine, sink, 4), Some(number(6)));

    // Value is tagged
    let made = engine.pure_cache.get(&make).and_then(|m| m.get(&PinId(0))).cloned()
        .or_else(|| engine.executed.iter().find(|t| t.node_id == make).and_then(|t| t.outputs.first().map(|(_, v)| v.clone())));
    assert_eq!(made.map(|v| v.get_type()), Some(point_type()));
}

#[test]
fn break_skips_missing_optional_field() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let source = NodeId::generate();
    engine.create_node(source, "test/point_source", &[]).unwrap();
    let brk = NodeId::generate();
    engine.create_node(brk, "type/break/test.point", &[]).unwrap();
    engine.connect_data(source, PinId(1), brk, PinId(0)).unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();

    // Break evaluates on demand; the point has no label
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(brk, PinId(3), probe, PinId(1)).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).expect("debug has run");
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);
    let trace = engine.executed.iter().find(|t| t.node_id == brk).expect("break was evaluated");
    assert!(trace.outputs.iter().any(|(p, _)| *p == PinId(1)));
    assert!(!trace.outputs.iter().any(|(p, _)| *p == PinId(3)));
}

#[test]
fn enum_input_values_are_validated() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine.set_input_value(sink, PinId(2), Some(DataValue::String("auto".into()))).unwrap();
    assert!(matches!(engine.set_input_value(sink, PinId(2), Some(DataValue::String("bogus".into()))), Err(EngineError::TypeMismatch(..))));
    assert!(engine.set_input_value(sink, PinId(2), Some(number(1))).is_err());
    // A struct cannot be set inline on a node
    assert!(engine.set_input_value(sink, PinId(1), Some(DataValue::String("x".into()))).is_err());
    // Value survived rejected attempts
    let model = engine.node_model(sink).unwrap();
    assert!(model.properties.contains(&("in:2".to_string(), DataValue::String("auto".into()))));
}

#[test]
fn graph_with_named_nodes_survives_save_and_load() {
    let Some(bytes) = plugin_bytes("test_plugin") else { return };
    let new_engine = || {
        let mut engine = TickEngine::new();
        engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
        engine
    };
    let mut engine = new_engine();
    let source = NodeId::generate();
    engine.create_node(source, "test/point_source", &[("x".into(), number(2)), ("y".into(), number(3))]).unwrap();
    let brk = NodeId::generate();
    engine.create_node(brk, "type/break/test.point", &[]).unwrap();
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine.connect_data(source, PinId(1), brk, PinId(0)).unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    engine.connect_data(brk, PinId(2), make, PinId(2)).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine.set_input_value(sink, PinId(2), Some(DataValue::String("auto".into()))).unwrap();

    // Roundtrip through JSON, as stored on disk
    let json = serde_json::to_string_pretty(&engine.export_graph()).unwrap();
    let graph: GraphModel = serde_json::from_str(&json).unwrap();
    let mut restored = new_engine();
    let warnings = restored.load_graph(&graph);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(restored.export_graph(), engine.export_graph());

    restored.trigger_node(source, PinId(0));
    restored.trigger_node(sink, PinId(0));
    restored.tick();
    assert_eq!(output(&restored, sink, 3), Some(DataValue::String("auto:2,3".into())));

    // Without the plugin, the typed node is not lost: a stub preserves its type, properties, and connections
    let mut bare = TickEngine::new();
    assert!(bare.load_graph(&graph).is_empty());
    assert_eq!(bare.export_graph(), graph);
    assert!(bare.node_unavailable(source).is_some());
}

// --- Required inputs ---

#[test]
fn make_requires_its_required_fields() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let def = engine.node_definition(make).unwrap();
    let required: Vec<_> = def.pins.iter().filter(|p| p.required).map(|p| p.id).collect();
    assert_eq!(required, [PinId(1), PinId(2)], "x and y are required, label is not");

    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(make, PinId(0), probe, PinId(1)).unwrap();
    engine.set_input_value(make, PinId(1), Some(number(1))).unwrap();

    // y is unset: no output value, one error logged (not repeated every tick)
    for _ in 0..3 {
        engine.trigger_node(probe, PinId(0));
        engine.tick();
    }
    let errors: Vec<_> = engine.host_state.logs.iter().filter(|(n, m)| *n == make && m.contains("Required input")).collect();
    assert_eq!(errors.len(), 1, "{:?}", engine.host_state.logs);
    assert!(errors[0].1.contains("\"Y\""));
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);

    // All required fields set: tagged object without the optional field
    engine.set_input_value(make, PinId(2), Some(number(2))).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    let Some((_, Some(DataValue::Object { fields, type_name }))) = trace.inputs.first() else { panic!("{:?}", trace.inputs) };
    assert_eq!(type_name.as_deref(), Some("test.point"));
    assert_eq!(fields.keys().map(String::as_str).collect::<Vec<_>>(), ["x", "y"]);

    // Optional field appears once provided
    engine.set_input_value(make, PinId(3), Some(DataValue::String("a".into()))).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    let Some((_, Some(DataValue::Object { fields, .. }))) = trace.inputs.first() else { panic!() };
    assert!(fields.contains_key("label"));
}

#[test]
fn connected_source_without_value_skips_silently() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let source = NodeId::generate();
    engine.create_node(source, "test/counter", &[]).unwrap(); // Count has no value yet
    engine.connect_data(source, PinId(1), make, PinId(1)).unwrap();
    engine.set_input_value(make, PinId(2), Some(number(2))).unwrap();
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(make, PinId(0), probe, PinId(1)).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().all(|(n, _)| *n != make), "{:?}", engine.host_state.logs);
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);
}

#[test]
fn plugin_required_pin_is_respected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    assert!(engine.node_definition(sink).unwrap().pins.iter().any(|p| p.id == PinId(1) && p.required));
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 3), None);
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == sink && m.contains("\"Point\"")), "{:?}", engine.host_state.logs);
}

#[test]
fn foreign_tagged_object_on_named_output_is_rejected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let bad = NodeId::generate();
    engine.create_node(bad, "test/bad_source", &[]).unwrap();
    engine.trigger_node(bad, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, bad, 1), None);
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == bad && m.contains("other.type")), "{:?}", engine.host_state.logs);

    // A valid output of the declared type passes through with its tag intact
    let good = NodeId::generate();
    engine.create_node(good, "test/point_source", &[]).unwrap();
    engine.trigger_node(good, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, good, 1).map(|v| v.get_type()), Some(point_type()));
}

// --- Plugin settings (configure) ---

fn test_settings(greeting: &str, token: &str) -> Vec<(String, DataValue)> {
    vec![
        ("greeting".into(), DataValue::String(greeting.into())),
        ("token".into(), DataValue::String(token.into())),
    ]
}

/// Engine with test_plugin loaded but disabled.
fn engine_with_disabled_test_plugin() -> Option<TickEngine> {
    let bytes = plugin_bytes("test_plugin")?;
    let mut engine = TickEngine::new();
    engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    Some(engine)
}

fn greet(engine: &mut TickEngine, node: NodeId) -> Option<DataValue> {
    engine.trigger_node(node, PinId(0));
    engine.tick();
    output(engine, node, 1)
}

#[test]
fn manifest_has_settings_schema() {
    let Some(engine) = engine_with_disabled_test_plugin() else { return };
    let manifest = engine.plugin_manifest("test_plugin").unwrap();
    let keys: Vec<_> = manifest.settings.iter().map(|s| (s.key.as_str(), s.required)).collect();
    assert_eq!(keys, [("greeting", false), ("token", true), ("mode", false), ("ws_url", false)]);
    assert_eq!(manifest.settings[0].default, Some(DataValue::String("Hello".into())));
    assert!(matches!(&manifest.settings[2].kind, SettingKind::Select(o) if o.len() == 2));
    // Added but not enabled: types are not registered yet
    assert!(!engine.plugin_active("test_plugin"));
    assert!(engine.registry.get_definition("test/greeter").is_none());
}

#[test]
fn configure_is_called_and_its_error_is_reported() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    let manifest = engine.plugin_manifest("test_plugin").unwrap().clone();

    // Enabling without token: configure rejects it, but the plugin remains operational
    let defaults = effective_settings(&manifest, &[]);
    engine.enable_plugin("test_plugin", &defaults).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin").map(|e| e.key).as_deref(), Some("test-plugin-error-no-token"));
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Hello|quiet|no-token".into())));

    // New settings are applied without a restart
    engine.configure_plugin("test_plugin", &effective_settings(&manifest, &test_settings("Hello", "good"))).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Hello|quiet|token".into())));

    // Plugin response is returned as an error and recorded
    let err = engine.configure_plugin("test_plugin", &test_settings("x", "bad")).unwrap_err();
    assert_eq!(err.key, "test-plugin-error-bad-token");
    assert_eq!(err.args, vec![("length".to_string(), "3".to_string())]);
    assert_eq!(err.fallback, "Invalid token");
    assert_eq!(engine.plugin_settings_error("test_plugin").map(|e| e.fallback), Some("Invalid token".to_string()));
    assert!(engine.plugin_active("test_plugin"));

    // Fixing the setting clears the error
    engine.configure_plugin("test_plugin", &test_settings("x", "ok")).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
}

#[test]
fn settings_survive_plugin_crash() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("Hello", "good")).unwrap();
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Hello|quiet|token".into())));

    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("Plugin crashed")));

    // Instance recreated: configure was re-run before recreating the node
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Hello|quiet|token".into())));
}

// --- Disabling and enabling a plugin ---

#[test]
fn disabling_plugin_keeps_nodes_as_stubs_and_enabling_restores_them() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();

    let source = NodeId::generate();
    engine.create_node(source, "test/point_source", &[("x".into(), number(2)), ("y".into(), number(3))]).unwrap();
    let brk = NodeId::generate();
    engine.create_node(brk, "type/break/test.point", &[]).unwrap();
    let sink_node = NodeId::generate();
    engine.create_node(sink_node, "test/point_sink", &[]).unwrap();
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let debug = NodeId::generate();
    engine.create_node(debug, "test/debug", &[]).unwrap();
    engine.connect_data(source, PinId(1), brk, PinId(0)).unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    engine.connect_data(brk, PinId(2), make, PinId(2)).unwrap();
    engine.connect_data(make, PinId(0), sink_node, PinId(1)).unwrap();
    engine.set_input_value(sink_node, PinId(2), Some(DataValue::String("manual".into()))).unwrap();
    engine.connect_data(sink_node, PinId(3), debug, PinId(1)).unwrap();
    let before = engine.export_graph();
    assert!(before.nodes.iter().find(|n| n.id == sink_node).unwrap().properties.iter().any(|(k, _)| k == "in:2"));

    let change = engine.disable_plugin("test_plugin").unwrap();
    assert_eq!(change.replaced.len(), 4, "source, brk, sink, make: {:?}", change.replaced);
    assert!(change.removed.data.is_empty());
    // Types unregistered from registries
    assert!(engine.registry.get_definition("test/point_source").is_none());
    assert!(engine.registry.get_definition("type/make/test.point").is_none());
    assert!(engine.types.get("test.point").is_none());
    assert!(!engine.plugin_active("test_plugin"));
    // Graph is identical: types, properties (including in:*), and connections
    assert_eq!(engine.export_graph(), before);
    // Stub: provides reason, rejects changes, does not execute
    assert_eq!(engine.node_unavailable(source).map(|e| e.fallback).as_deref(), Some("plugin test_plugin unavailable"));
    assert!(engine.node_unavailable(debug).is_none());
    assert!(matches!(engine.set_node_property(source, "x", number(9)), Err(EngineError::PluginUnavailable(_))));
    assert!(matches!(engine.set_input_value(sink_node, PinId(2), None), Err(EngineError::PluginUnavailable(_))));
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == source && m.contains("plugin test_plugin unavailable")));
    // Definition is no longer in the palette, but the node still retains its pins
    assert_eq!(engine.node_definition(source).unwrap().pins.len(), 2);

    // Disabling again is an error; a graph saved while disabled is also preserved intact
    assert!(engine.disable_plugin("test_plugin").is_err());
    let json = serde_json::to_string(&engine.export_graph()).unwrap();
    let mut reloaded = TickEngine::new();
    assert!(reloaded.load_graph(&serde_json::from_str(&json).unwrap()).is_empty());
    assert_eq!(reloaded.export_graph(), before);

    // Enabling: stubs become real nodes again, connections remain intact
    let change = engine.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();
    assert_eq!(change.replaced.len(), 4);
    assert!(change.removed.data.is_empty() && change.removed.exec.is_empty());
    assert!(engine.node_unavailable(source).is_none());
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink_node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink_node, 3), Some(DataValue::String("manual:2,3".into())));

    // A graph reloaded with stubs also revives once the plugin becomes available
    let bytes = plugin_bytes("test_plugin").unwrap();
    reloaded.add_plugin(&bytes, None, |_| vec![]).unwrap();
    let change = reloaded.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();
    assert_eq!(change.replaced.len(), 4);
    assert_eq!(reloaded.export_graph(), before);
    reloaded.trigger_node(source, PinId(0));
    reloaded.trigger_node(sink_node, PinId(0));
    reloaded.tick();
    assert_eq!(output(&reloaded, sink_node, 3), Some(DataValue::String("manual:2,3".into())));
}

#[test]
fn enabling_detaches_connections_that_became_invalid() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    let counter = NodeId::generate();
    let debug = NodeId::generate();
    let ghost_link = ConnectionModel { src_node: counter, src_pin: PinId(9), dst_node: debug, dst_pin: PinId(0) };
    let graph = GraphModel {
        nodes: vec![
            NodeModel { id: counter, type_name: "test/counter".into(), properties: vec![("step".into(), number(2))], position: Position::default(), pins: None, size: None },
            NodeModel { id: debug, type_name: "test/debug".into(), properties: vec![("prefix".into(), DataValue::String(String::new()))], position: Position::default(), pins: None, size: None },
        ],
        // Pin 9 was inferred by the stub from the connection, but the real counter node has no such pin
        exec_connections: vec![ghost_link],
        data_connections: vec![ConnectionModel { src_node: counter, src_pin: PinId(1), dst_node: debug, dst_pin: PinId(1) }],
        wire_routes: Vec::new(), detached_connections: Vec::new(),
    };
    let mut graph = graph;
    graph.nodes.sort_by_key(|n| n.id.0);
    assert!(engine.load_graph(&graph).is_empty());
    let graph = engine.export_graph();
    assert!(graph.nodes.iter().all(|n| n.pins.is_some()));

    let change = engine.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();
    assert_eq!(change.replaced, vec![counter]);
    assert!(change.removed.exec.is_empty());
    assert_eq!(change.detached.len(), 1);
    assert_eq!(change.detached[0].connection, ghost_link);
    assert_eq!(change.detached[0].end, DetachedEnd::Source);
    assert!(!engine.frozen_changes(counter).is_empty());
    assert!(change.removed.data.is_empty());
    let after = engine.export_graph();
    assert!(after.exec_connections.is_empty());
    assert_eq!(after.data_connections, graph.data_connections);
}

#[test]
fn unknown_node_type_survives_load_and_save() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let c = NodeId::generate();
    let graph = GraphModel {
        nodes: vec![
            NodeModel { id: a, type_name: "ghost/source".into(), properties: vec![("k".into(), number(1)), ("in:3".into(), DataValue::String("x".into()))], position: Position { x: 5.0, y: 6.0 }, pins: None, size: None },
            NodeModel { id: b, type_name: "test/debug".into(), properties: vec![("prefix".into(), DataValue::String(String::new()))], position: Position::default(), pins: None, size: None },
            NodeModel { id: c, type_name: "ghost/sink".into(), properties: vec![], position: Position::default(), pins: None, size: None },
        ],
        exec_connections: vec![ConnectionModel { src_node: a, src_pin: PinId(0), dst_node: c, dst_pin: PinId(4) }],
        data_connections: vec![
            ConnectionModel { src_node: a, src_pin: PinId(1), dst_node: b, dst_pin: PinId(1) },
            ConnectionModel { src_node: a, src_pin: PinId(2), dst_node: c, dst_pin: PinId(5) },
        ],
        wire_routes: Vec::new(), detached_connections: Vec::new(),
    };
    let mut engine = TickEngine::new();
    assert!(engine.load_graph(&graph).is_empty());
    let mut expected = graph.clone();
    expected.nodes.iter_mut().for_each(|n| {
        n.position = Position::default();
        n.pins = engine.node_model(n.id).unwrap().pins;
    });
    assert_eq!(engine.export_graph().nodes.len(), 3);
    let mut exported = engine.export_graph();
    exported.nodes.sort_by_key(|n| n.id.0);
    expected.nodes.sort_by_key(|n| n.id.0);
    assert_eq!(exported.nodes, expected.nodes);
    assert_eq!(exported.exec_connections, graph.exec_connections);
    let mut want = graph.data_connections.clone();
    want.sort_by_key(|c| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0));
    assert_eq!(exported.data_connections, want);
    // Pins inferred from connections; the data pin type came from the other end (Debug.Value is Any)
    let pins = engine.node_definition(a).unwrap().pins;
    assert_eq!(pins.len(), 3);
    assert!(pins.iter().any(|p| p.id == PinId(0) && p.direction == PinDirection::Output && p.pin_type == PinType::Exec));
}

// --- Translations shipped by plugins ---

fn locale_text(translations: &Translations, locale: &str) -> String {
    translations.iter().find(|(l, _)| l == locale).map(|(_, t)| t.clone()).unwrap_or_default()
}

#[test]
fn plugin_translations_are_validated_and_delivered() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    // A disabled plugin contributes nothing; the core resources are always there
    let before = engine.translations();
    assert!(locale_text(&before, "uk").contains("node-const-title"));
    assert!(!locale_text(&before, "uk").contains("node-test-greeter"));

    engine.enable_plugin("test_plugin", &test_settings("Hello", "good")).unwrap();
    let after = engine.translations();
    let uk = locale_text(&after, "uk");
    assert!(uk.contains("node-const-title"), "core resource first");
    assert!(uk.contains("node-test-greeter-description") && uk.contains("type-test-mode-value-manual"), "{uk}");
    assert!(uk.contains("test-plugin-error-bad-token"));
    assert!(uk.contains("setting-test-plugin-mode-option-loud"));
    // `node-math-add-title` is not the plugin's to define: dropped, the core text stays
    assert!(!uk.contains("HIJACKED"), "{uk}");
    assert!(uk.contains("node-math-add-title"), "the core definition is kept");
    // A resource with a syntax error is rejected as a whole; an unknown locale is passed on
    assert!(after.iter().all(|(l, _)| l != "de"), "{:?}", after.iter().map(|(l, _)| l).collect::<Vec<_>>());
    assert!(locale_text(&after, "zz").contains("Counter (zz)"));

    // Disabling removes the plugin's resources again
    engine.disable_plugin("test_plugin").unwrap();
    let off = engine.translations();
    assert!(!locale_text(&off, "uk").contains("node-test-greeter"));
    assert!(locale_text(&off, "zz").is_empty());
}

#[test]
fn configure_errors_carry_translatable_messages() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("x", "good")).unwrap();

    // Own namespace: key and arguments pass through
    let e = engine.configure_plugin("test_plugin", &test_settings("x", "bad")).unwrap_err();
    assert_eq!((e.key.as_str(), e.fallback.as_str()), ("test-plugin-error-bad-token", "Invalid token"));
    assert_eq!(e.args, vec![("length".to_string(), "3".to_string())]);

    // A key outside the plugin namespace is dropped, the English fallback stays
    let e = engine.configure_plugin("test_plugin", &test_settings("x", "foreign")).unwrap_err();
    assert_eq!((e.key.as_str(), e.fallback.as_str()), ("", "Foreign key"));

    // A plain `String` error has no key
    let e = engine.configure_plugin("test_plugin", &test_settings("x", "plain")).unwrap_err();
    assert_eq!((e.key.as_str(), e.fallback.as_str()), ("", "Plain error"));
}

#[test]
fn engine_errors_have_ui_messages() {
    let e = EngineError::UnknownNodeType("x/y".into()).ui_message();
    assert_eq!((e.key.as_str(), e.args.clone()), ("error-unknown-node-type", vec![("type".to_string(), "x/y".to_string())]));
    assert_eq!(e.fallback, "Unknown node type: x/y");
}

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
    let server = EchoServer { port: 0, opened: Arc::default(), closed: Arc::default(), header: Arc::default() };
    let (opened, closed, header) = (server.opened.clone(), server.closed.clone(), server.header.clone());
    let (port_tx, port_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            port_tx.send(listener.local_addr().unwrap().port()).unwrap();
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let (opened, closed, header) = (opened.clone(), closed.clone(), header.clone());
                tokio::spawn(async move {
                    let callback = |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
                        *header.lock().unwrap() = req.headers().get("x-test").and_then(|v| v.to_str().ok()).map(String::from);
                        Ok(resp)
                    };
                    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, callback).await else { return };
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
    EchoServer { port: port_rx.recv().unwrap(), ..server }
}

fn ws_settings(url: &str) -> Vec<(String, DataValue)> {
    let mut settings = test_settings("Hi", "good");
    settings.push(("ws_url".into(), DataValue::String(url.into())));
    settings
}

fn engine_with_ws(url: &str, hosts: Vec<String>, runtime: &tokio::runtime::Runtime) -> Option<TickEngine> {
    let bytes = plugin_bytes("test_plugin")?;
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine.add_plugin(&bytes, None, |_| vec![Capability::WebSocket { allowed_hosts: hosts }]).unwrap();
    engine.enable_plugin("test_plugin", &ws_settings(url)).unwrap();
    Some(engine)
}

fn status(engine: &TickEngine, node: NodeId) -> Option<DataValue> {
    output(engine, node, 4)
}

#[test]
fn websocket_roundtrip_reaches_node_outputs_through_plugin_tick() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(&format!("ws://127.0.0.1:{}/", server.port), vec!["127.0.0.1".into()], &runtime) else { return };
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);

    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    constant(&mut engine, DataValue::String("hello".into()), probe, 1);
    let received = sink(&mut engine, probe, 2);

    // The plugin opened the connection in `configure`; `opened` arrives in plugin-tick and is
    // pushed to the node without any execute.
    run_until(&mut engine, |e| status(e, probe) == Some(DataValue::String("open".into())));
    assert_eq!(server.header.lock().unwrap().as_deref(), Some("1"), "custom headers reach the server");

    engine.trigger_node(probe, PinId(0));
    run_until(&mut engine, |e| output(e, probe, 3).is_some());
    assert_eq!(output(&engine, probe, 3), Some(DataValue::String("echo:hello".into())));
    // The exec pulse from the emission queues the downstream node, and the trace records it
    run_until(&mut engine, |e| e.execution_queue.contains(&(received, PinId(0))) || e.executed.iter().any(|t| t.node_id == received));
}

#[test]
fn websocket_works_when_the_runtime_is_set_after_the_plugin_is_loaded() {
    // The server loads plugins before the engine thread gets its runtime: the existing instance must
    // pick the runtime up later instead of failing with "running without a tokio runtime".
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(bytes) = plugin_bytes("test_plugin") else { return };
    let mut engine = TickEngine::new();
    engine.add_plugin(&bytes, None, |_| vec![Capability::WebSocket { allowed_hosts: vec!["*".into()] }]).unwrap();
    engine.enable_plugin("test_plugin", &ws_settings("")).unwrap();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine.configure_plugin("test_plugin", &ws_settings(&format!("ws://127.0.0.1:{}/", server.port))).unwrap();
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    run_until(&mut engine, |e| status(e, probe) == Some(DataValue::String("open".into())));
}

#[test]
fn websocket_emission_trace_is_recorded() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(&format!("ws://127.0.0.1:{}/", server.port), vec!["*".into()], &runtime) else { return };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    let mut traced = false;
    for _ in 0..200 {
        engine.tick();
        traced |= engine.executed.iter().any(|t| t.node_id == probe && t.outputs.iter().any(|(p, _)| p.0 == 4));
        if traced { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(traced, "the emission is recorded like an execution");
}

#[test]
fn websocket_to_a_host_outside_the_capability_is_denied() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    // Only example.com is allowed: connecting to 127.0.0.1 fails in `configure`, and nothing connects
    let Some(engine) = engine_with_ws(&format!("ws://127.0.0.1:{}/", server.port), vec!["example.com".into()], &runtime) else { return };
    let error = engine.plugin_settings_error("test_plugin").expect("configure reports the denial");
    assert!(error.fallback.contains("not allowed"), "{error:?}");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(server.opened.load(Ordering::SeqCst), 0);

    // Wrong scheme is rejected as well
    let Some(mut engine) = engine_with_ws("", vec!["*".into()], &runtime) else { return };
    let err = engine.configure_plugin("test_plugin", &ws_settings("http://127.0.0.1:1/")).unwrap_err();
    assert!(err.fallback.contains("Unsupported scheme"), "{err:?}");
}

#[test]
fn websocket_connection_closes_when_the_plugin_is_disabled() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(&format!("ws://127.0.0.1:{}/", server.port), vec!["*".into()], &runtime) else { return };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    run_until(&mut engine, |e| status(e, probe) == Some(DataValue::String("open".into())));
    assert_eq!(server.closed.load(Ordering::SeqCst), 0);

    engine.disable_plugin("test_plugin").unwrap();
    for _ in 0..100 {
        if server.closed.load(Ordering::SeqCst) == 1 { return; }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("the server did not observe the connection closing");
}

#[test]
fn websocket_reconnects_after_the_plugin_crashes() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let server = echo_server();
    let Some(mut engine) = engine_with_ws(&format!("ws://127.0.0.1:{}/", server.port), vec!["*".into()], &runtime) else { return };
    let probe = NodeId::generate();
    engine.create_node(probe, "test/ws_probe", &[]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    run_until(&mut engine, |e| status(e, probe) == Some(DataValue::String("open".into())));

    engine.trigger_node(crash, PinId(0));
    // The old connection is closed with the dead instance; configure runs again on the new one and reconnects
    run_until(&mut engine, |_| server.opened.load(Ordering::SeqCst) == 2 && server.closed.load(Ordering::SeqCst) >= 1);
    // The recreated probe node (same graph id) receives the new connection's events
    constant(&mut engine, DataValue::String("again".into()), probe, 1);
    engine.trigger_node(probe, PinId(0));
    run_until(&mut engine, |e| output(e, probe, 3) == Some(DataValue::String("echo:again".into())));
}

// --- Home Assistant plugin against a fake Home Assistant WebSocket server ---

const HA_TOKEN: &str = "test-token";

fn ha_services() -> serde_json::Value {
    serde_json::json!({
        "light":{"turn_on":{"name":"Turn on", "target":{"entity":[{"domain":"light"}]}, "fields":{
            "brightness":{"name":"Brightness","required":true,"selector":{"number":{"min":0,"max":255}}},
            "effect":{"selector":{"select":{"options":["none","rainbow"]}}},
            "rgb_color":{"selector":{"color_rgb":null}},
            "transition":{"advanced":true,"selector":{"number":{}}}
        }}},
        "weather":{"get_forecasts":{"name":"Get forecasts", "target":{"entity":[{"domain":"weather"}]}, "fields":{
            "type":{"selector":{"select":{"options":["daily","hourly"]}}}
        },"response":{"optional":false}}},
        "homeassistant":{"turn_on":{"name":"Turn on", "target":{}}}
    })
}

/// A minimal Home Assistant: the auth handshake, `subscribe_trigger` / `subscribe_entities` /
/// `unsubscribe_events` / `call_service` as `ha-core-client` uses them, and pushed state changes.
/// Like `echo_server`, it runs in its own runtime on its own thread.
struct FakeHa {
    port: u16,
    /// Message id of each entity's `subscribe_trigger`.
    subs: Arc<std::sync::Mutex<std::collections::HashMap<String, u64>>>,
    /// Current state of each entity (default "off").
    states: Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
    unsubscribed: Arc<AtomicUsize>,
    connections: Arc<AtomicUsize>,
    /// `call_service` requests as received.
    calls: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// Automation trigger requests, including their message ids and typed configurations.
    trigger_requests: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// What `get_states` answers.
    entity_list: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
    /// Frames to push to every connected client.
    push: tokio::sync::broadcast::Sender<String>,
}

impl FakeHa {
    fn start() -> FakeHa {
        use futures::{SinkExt, StreamExt};
        use serde_json::{Value, json};
        let (push, _) = tokio::sync::broadcast::channel(64);
        let server = FakeHa {
            port: 0,
            subs: Arc::default(),
            states: Arc::default(),
            unsubscribed: Arc::default(),
            connections: Arc::default(),
            calls: Arc::default(),
            entity_list: Arc::default(),
            trigger_requests: Arc::default(),
            push,
        };
        let (subs, states, unsubscribed, connections, calls, push, entity_list, trigger_requests) =
            (server.subs.clone(), server.states.clone(), server.unsubscribed.clone(), server.connections.clone(), server.calls.clone(), server.push.clone(), server.entity_list.clone(), server.trigger_requests.clone());
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
            runtime.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                port_tx.send(listener.local_addr().unwrap().port()).unwrap();
                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    let (subs, states, unsubscribed, connections, calls, entity_list, trigger_requests) = (subs.clone(), states.clone(), unsubscribed.clone(), connections.clone(), calls.clone(), entity_list.clone(), trigger_requests.clone());
                    let mut pushed = push.subscribe();
                    tokio::spawn(async move {
                        use tokio_tungstenite::tungstenite::Message;
                        let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else { return };
                        connections.fetch_add(1, Ordering::SeqCst);
                        let _ = ws.send(json!({"type": "auth_required", "ha_version": "2026.10.0"}).to_string().into()).await;
                        let mut authenticated = false;
                        loop {
                            let text = tokio::select! {
                                message = ws.next() => match message {
                                    Some(Ok(Message::Text(text))) => text.to_string(),
                                    Some(Ok(_)) => continue,
                                    _ => break,
                                },
                                frame = pushed.recv() => {
                                    if let Ok(frame) = frame && authenticated {
                                        let _ = ws.send(frame.into()).await;
                                    }
                                    continue;
                                }
                            };
                            let request: Value = serde_json::from_str(&text).unwrap();
                            if !authenticated {
                                let ok = request["type"] == "auth" && request["access_token"] == HA_TOKEN;
                                let reply = if ok { json!({"type": "auth_ok", "ha_version": "2026.10.0"}) } else { json!({"type": "auth_invalid", "message": "Invalid access token"}) };
                                let _ = ws.send(reply.to_string().into()).await;
                                authenticated = ok;
                                continue;
                            }
                            let id = request["id"].clone();
                            let mut replies = vec![json!({"id": id, "type": "result", "success": true, "result": null})];
                            match request["type"].as_str().unwrap_or_default() {
                                "subscribe_trigger" => {
                                    trigger_requests.lock().unwrap().push(request.clone());
                                    let entity = &request["trigger"]["entity_id"];
                                    if let Some(entity) = entity.as_str().or_else(|| entity[0].as_str()) {
                                        subs.lock().unwrap().insert(entity.to_string(), id.as_u64().unwrap());
                                    }
                                    if request["trigger"]["event_type"][0] == "reject" {
                                        replies = vec![json!({"id":id,"type":"result","success":false,"error":{"code":"invalid_format","message":"Trigger rejected by fake HA"}})];
                                    }
                                }
                                "subscribe_events" if request["event_type"] == "state_changed" => {
                                    subs.lock().unwrap().insert("__cache".into(), id.as_u64().unwrap());
                                }
                                "get_config" => {
                                    replies = vec![json!({"id":id,"type":"result","success":true,"result":{"time_zone":"Europe/Kyiv"}})];
                                }
                                "render_template" => {
                                    let value = if request["template"].as_str().unwrap().contains("utcoffset") { json!(7200) }
                                        else if request["template"] == "{{ true }}" { json!(true) } else { json!(false) };
                                    replies.push(json!({"id":id,"type":"event","event":{"result":value,"listeners":{}}}));
                                }
                                "subscribe_entities" => {
                                    let entity = request["entity_ids"][0].as_str().unwrap();
                                    let state = states.lock().unwrap().get(entity).cloned().unwrap_or_else(|| "off".into());
                                    replies.push(json!({"id": id, "type": "event", "event": {"a": {entity: {"s": state, "a": {}, "c": "01X", "lc": 1.0, "lu": 1.0}}}}));
                                }
                                "unsubscribe_events" => {
                                    unsubscribed.fetch_add(1, Ordering::SeqCst);
                                }
                                "get_services" => {
                                    replies = vec![json!({"id":id,"type":"result","success":true,"result":ha_services()})];
                                }
                                "get_states" => {
                                    replies = vec![json!({"id": id, "type": "result", "success": true, "result": entity_list.lock().unwrap().clone()})];
                                }
                                "call_service" => {
                                    calls.lock().unwrap().push(request.clone());
                                    if request["return_response"] == true {
                                        replies = vec![json!({"id":id,"type":"result","success":true,"result":{"context":{"id":"call"},"response":{"weather.home":{"forecast":[{"temperature":21}]}}}})];
                                    }
                                    if request["domain"] == "broken" {
                                        replies = vec![json!({"id": id, "type": "result", "success": false, "error": {"code": "not_found", "message": "Service not found."}})];
                                    }
                                }
                                _ => {}
                            }
                            for reply in replies {
                                let _ = ws.send(reply.to_string().into()).await;
                            }
                        }
                    });
                }
            });
        });
        FakeHa { port: port_rx.recv().unwrap(), ..server }
    }

    fn url(&self) -> String {
        format!("ws://127.0.0.1:{}/api/websocket", self.port)
    }

    /// Pushes HA's trigger-variable envelope to a particular subscription.
    fn fire(&self, id: u64, trigger: serde_json::Value) {
        let _ = self.push.send(serde_json::json!({"id":id,"type":"event","event":{"variables":{"trigger":trigger},"context":{"id":"test"}}}).to_string());
    }

    /// Pushes a state change of `entity`, as the entity's `state` trigger subscription reports it.
    fn change(&self, entity: &str, from: &str, to: &str) {
        let state = |s: &str| serde_json::json!({
            "entity_id": entity, "state": s, "attributes": {"friendly_name": "Kitchen", "brightness": 120},
            "last_changed": "2026-10-30T23:15:42.500000+02:00",
            "last_reported": "2026-10-30T23:15:42.500000+02:00",
            "last_updated": "2026-10-30T23:15:42.500000+02:00",
            "context": {"id": "01X", "parent_id": null, "user_id": null}
        });
        self.states.lock().unwrap().insert(entity.to_string(), to.to_string());
        if let Some(id) = self.subs.lock().unwrap().get("__cache").copied() {
            let _ = self.push.send(serde_json::json!({"id":id,"type":"event","event":{"event_type":"state_changed","data":{"entity_id":entity,"old_state":state(from),"new_state":state(to)},"origin":"LOCAL","time_fired":"2026-10-30T10:00:00Z","context":{"id":"cache"}}}).to_string());
        }
        let Some(id) = self.subs.lock().unwrap().get(entity).copied() else { return };
        let event = serde_json::json!({"id": id, "type": "event", "event": {"variables": {"trigger": {
            "id": "0", "idx": "0", "alias": null, "platform": "state", "entity_id": entity,
            "from_state": state(from), "to_state": state(to), "for": null, "attribute": null,
        }}, "context": {"id": "01X", "parent_id": null, "user_id": null}}});
        let _ = self.push.send(event.to_string());
    }
}

fn ha_settings(url: &str, token: &str) -> Vec<(String, DataValue)> {
    vec![("url".into(), DataValue::String(url.into())), ("token".into(), DataValue::String(token.into()))]
}

fn ha_engine(ha: &FakeHa, token: &str, runtime: &tokio::runtime::Runtime) -> Option<TickEngine> {
    let bytes = plugin_bytes("ha_plugin")?;
    let mut engine = TickEngine::new();
    engine.host_state.runtime = Some(runtime.handle().clone());
    engine.add_plugin(&bytes, None, |_| vec![Capability::WebSocket { allowed_hosts: vec!["127.0.0.1".into()] }]).unwrap();
    engine.enable_plugin("home_assistant", &ha_settings(&ha.url(), token)).unwrap();
    Some(engine)
}

fn field_text(value: &DataValue, name: &str) -> Option<String> {
    match value {
        DataValue::Object { fields, .. } => match &fields.get(name)?.value {
            DataValue::String(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// Ticks until the sink has executed `count` times in total (starting from `*seen`), returns the new total.
fn count_executions(engine: &mut TickEngine, sink: NodeId, mut seen: usize, until: usize) -> usize {
    for _ in 0..300 {
        engine.tick();
        seen += engine.executed.iter().filter(|t| t.node_id == sink).count();
        if seen >= until {
            return seen;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the sink ran {seen} times, expected {until}");
}

#[test]
fn ha_plugin_declares_its_nodes_and_type() {
    let Some(bytes) = plugin_bytes("ha_plugin") else { return };
    let mut engine = TickEngine::new();
    let manifest = engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    assert_eq!(manifest.name, "home_assistant");
    assert!(manifest.settings.iter().any(|s| s.key == "token" && s.is_secret() && s.required));
    assert!(manifest.settings.iter().any(|s| s.key == "url" && s.default == Some(DataValue::String("ws://homeassistant.local:8123/api/websocket".into()))));
    engine.enable_plugin("home_assistant", &[("token".into(), DataValue::String("t".into()))]).unwrap();
    for node in [
        "ha/action", "ha/entity", "ha/state_changed", "ha/get_state", "ha/call_service",
        "ha/trigger_state", "ha/trigger_numeric_state", "ha/trigger_time", "ha/trigger_time_pattern",
        "ha/trigger_sun", "ha/trigger_zone", "ha/trigger_event", "ha/trigger_homeassistant",
        "ha/trigger_mqtt", "ha/trigger_webhook", "ha/trigger_calendar", "ha/trigger_tag",
        "ha/trigger_conversation", "ha/trigger_template",
        "ha/condition_state", "ha/condition_numeric_state", "ha/condition_time",
        "ha/condition_sun", "ha/condition_zone", "ha/condition_template",
    ] {
        let definition = engine.registry.get_definition(node).unwrap();
        assert_eq!(definition.category, "Home Assistant");
        assert!(!definition.help.is_empty());
    }
    assert!(engine.types.get("ha.state").is_some());
    // The property schema: a picker over the plugin's own `entities` source and a drop-down
    let def = engine.registry.get_definition("ha/state_changed").unwrap();
    assert_eq!(def.properties.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["entity_id", "mode"]);
    assert_eq!(def.properties[0].kind, PropertyKind::Dynamic { source: "entities".into(), plugin: "home_assistant".into() });
    assert!(matches!(&def.properties[1].kind, PropertyKind::Select(o) if o.iter().map(|(v, _)| v.as_str()).collect::<Vec<_>>() == ["every", "latest"]));
    // A node added from the palette (no properties) exposes the plugin's defaults, so the editor shows them
    let node = NodeId::generate();
    engine.create_node(node, "ha/state_changed", &[]).unwrap();
    let properties = &engine.export_graph().nodes.into_iter().find(|n| n.id == node).unwrap().properties;
    assert!(properties.iter().any(|(k, v)| k == "entity_id" && *v == DataValue::String(String::new())), "{properties:?}");
    assert!(properties.iter().any(|(k, v)| k == "mode" && *v == DataValue::String("every".into())), "{properties:?}");
    // Every message the plugin ships is kept by the host (nothing is dropped as foreign)
    let translations = engine.translations();
    let merged = &translations.iter().find(|(locale, _)| locale == "uk").expect("the uk resource").1;
    let shipped = include_str!("../../ha-plugin/locales/uk.ftl");
    let ids: Vec<_> = shipped.lines().filter(|l| l.starts_with(|c: char| c.is_ascii_lowercase())).filter_map(|l| l.split_once(" =").map(|(id, _)| id)).collect();
    assert!(ids.len() >= 40, "{} ids", ids.len());
    for id in ids {
        assert!(merged.contains(&format!("{id} =")), "{id} is missing from the merged translations");
    }
}

#[test]
fn ha_plugin_state_changes_and_service_calls_work_end_to_end() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    assert_eq!(engine.plugin_settings_error("home_assistant"), None);

    let watch = NodeId::generate();
    engine.create_node(watch, "ha/state_changed", &[("entity_id".into(), DataValue::String("light.kitchen".into()))]).unwrap();
    let changed = sink(&mut engine, watch, 0);

    // Wait until the subscription is established (the plugin has read the baseline and cleaned up)
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) >= 1);
    assert_eq!(ha.connections.load(Ordering::SeqCst), 1);

    ha.change("light.kitchen", "off", "on");
    let mut runs = count_executions(&mut engine, changed, 0, 1);
    let new = output(&engine, watch, 1).unwrap();
    let old = output(&engine, watch, 2).unwrap();
    assert_eq!(new.get_type(), DataType::Named("ha.state".into()));
    assert_eq!(field_text(&new, "entity_id").as_deref(), Some("light.kitchen"));
    assert_eq!(field_text(&new, "state").as_deref(), Some("on"));
    assert_eq!(field_text(&old, "state").as_deref(), Some("off"));
    let DataValue::Object { fields, .. } = &new else { panic!() };
    // 23:15:42.5 at +02:00 is 21:15:42.5 UTC
    let expected = text::parse_datetime("2026-10-30T21:15:42.500Z").unwrap();
    assert_eq!(fields["last_changed"].value, DataValue::DateTime(expected));
    let DataValue::Object { fields: attributes, .. } = &fields["attributes"].value else { panic!("attributes") };
    assert_eq!(attributes["friendly_name"].value, DataValue::String("Kitchen".into()));
    assert_eq!(attributes["brightness"].value, number(120));

    // `every` delivers each change, one signal per change, in order
    ha.change("light.kitchen", "on", "off");
    ha.change("light.kitchen", "off", "on");
    runs = count_executions(&mut engine, changed, runs, 3);
    for _ in 0..20 {
        engine.tick();
        runs += engine.executed.iter().filter(|t| t.node_id == changed).count();
    }
    assert_eq!(runs, 3, "exactly one signal per change");
    assert_eq!(field_text(&output(&engine, watch, 1).unwrap(), "state").as_deref(), Some("on"));
    assert_eq!(field_text(&output(&engine, watch, 2).unwrap(), "state").as_deref(), Some("off"));

    // Calling a service: domain, service, target and data reach the server
    let call = NodeId::generate();
    engine.create_node(call, "ha/call_service", &[]).unwrap();
    constant(&mut engine, DataValue::String("light".into()), call, 1);
    constant(&mut engine, DataValue::String("turn_on".into()), call, 2);
    constant(&mut engine, DataValue::String("light.kitchen".into()), call, 3);
    let data = NodeId::generate();
    engine.create_node(data, "object/create", &[]).unwrap();
    let field = NodeId::generate();
    engine.create_node(field, "object/set_field", &[("field_type".into(), DataValue::String("number".into()))]).unwrap();
    engine.connect_data(data, PinId(0), field, PinId(0)).unwrap();
    constant(&mut engine, DataValue::String("brightness".into()), field, 1);
    constant(&mut engine, number(50), field, 2);
    engine.connect_data(field, PinId(3), call, PinId(4)).unwrap();
    let done = sink(&mut engine, call, 5);
    let failed = sink(&mut engine, call, 6);
    engine.trigger_node(call, PinId(0));
    count_executions(&mut engine, done, 0, 1);
    let calls = ha.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        (&calls[0]["domain"], &calls[0]["service"], &calls[0]["target"], &calls[0]["service_data"]),
        (&serde_json::json!("light"), &serde_json::json!("turn_on"), &serde_json::json!({"entity_id": ["light.kitchen"]}), &serde_json::json!({"brightness": 50})),
    );
    assert_eq!(output(&engine, call, 7), Some(DataValue::String(String::new())));

    // A rejected call goes to Failed with the reason
    let broken = NodeId::generate();
    engine.create_node(broken, "ha/call_service", &[
        ("in:1".into(), DataValue::String("broken".into())),
        ("in:2".into(), DataValue::String("x".into())),
    ]).unwrap();
    let broken_failed = sink(&mut engine, broken, 6);
    engine.trigger_node(broken, PinId(0));
    count_executions(&mut engine, broken_failed, 0, 1);
    let error = output(&engine, broken, 7).unwrap();
    assert!(matches!(&error, DataValue::String(e) if e.contains("Service not found")), "{error:?}");
    assert_eq!(engine.executed.iter().filter(|t| t.node_id == failed).count(), 0);

    // Reading a state
    let get = NodeId::generate();
    engine.create_node(get, "ha/get_state", &[("in:1".into(), DataValue::String("light.kitchen".into()))]).unwrap();
    let got = sink(&mut engine, get, 2);
    engine.trigger_node(get, PinId(0));
    count_executions(&mut engine, got, 0, 1);
    assert_eq!(field_text(&output(&engine, get, 4).unwrap(), "state").as_deref(), Some("on"));
}

#[test]
fn ha_plugin_requests_fail_when_the_token_is_wrong_and_nothing_hangs() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, "wrong", &runtime) else { return };
    let get = NodeId::generate();
    engine.create_node(get, "ha/get_state", &[("in:1".into(), DataValue::String("light.kitchen".into()))]).unwrap();
    let failed = sink(&mut engine, get, 3);
    // Let the authentication be rejected, then ask: the request fails at once
    run_until(&mut engine, |_| ha.connections.load(Ordering::SeqCst) == 1);
    for _ in 0..30 {
        engine.tick();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    engine.trigger_node(get, PinId(0));
    count_executions(&mut engine, failed, 0, 1);
    let error = output(&engine, get, 5).unwrap();
    assert!(matches!(&error, DataValue::String(e) if !e.is_empty()), "{error:?}");
    assert_eq!(ha.connections.load(Ordering::SeqCst), 1, "a rejected token is never retried");
}

#[test]
fn ha_plugin_without_a_token_reports_a_settings_error() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, "", &runtime) else { return };
    let error = engine.plugin_settings_error("home_assistant").expect("a missing token is reported");
    assert_eq!(error.key, "home-assistant-error-no-token");
    let error = engine.configure_plugin("home_assistant", &ha_settings("http://127.0.0.1:1/", "t")).unwrap_err();
    assert_eq!(error.key, "home-assistant-error-bad-url");
    assert_eq!(ha.connections.load(Ordering::SeqCst), 0);
}

// --- Property schema and dynamic options ---

fn option_set<'a>(sets: &'a [PropertyOptionSet], source: &str) -> Option<&'a PropertyOptionSet> {
    sets.iter().find(|s| s.plugin == "test_plugin" && s.source == source)
}

#[test]
fn property_schema_and_dynamic_options_follow_the_plugin_lifecycle() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    assert!(engine.property_options().is_empty(), "nothing is published before the plugin is enabled");
    engine.enable_plugin("test_plugin", &test_settings("hi", "t")).unwrap();

    // The schema reaches the definition; the host fills the owning plugin into a dynamic property
    let def = engine.registry.get_definition("test/picker").unwrap().clone();
    assert_eq!(def.properties.len(), 2);
    assert_eq!(def.properties[0].kind, PropertyKind::Dynamic { source: "greetings".into(), plugin: "test_plugin".into() });
    assert!(matches!(&def.properties[1].kind, PropertyKind::Select(o) if o == &[("a".to_string(), "Option A".to_string()), ("b".to_string(), "Option B".to_string())]));
    assert_eq!(def.properties[1].description, "Static options");
    let node = NodeId::generate();
    engine.create_node(node, "test/picker", &[("choice".into(), DataValue::String("hi".into()))]).unwrap();
    assert_eq!(engine.node_definition(node).unwrap().properties, def.properties);

    // Options published from `configure` are stored per (plugin, source)
    let sets = engine.property_options();
    let greetings = option_set(&sets, "greetings").expect("published by configure");
    assert_eq!(greetings.options, vec![PropertyOption { value: "hi".into(), label: "HI".into(), detail: "detail hi".into(), icon: Some("mdi:hand-wave".into()) }]);
    let changes = engine.take_property_option_changes();
    assert_eq!(option_set(&changes, "greetings").map(|s| s.options.len()), Some(1));
    assert!(engine.take_property_option_changes().is_empty(), "changes are reported once");

    // Re-configuring with the same value does not mark anything as changed; a new value does
    engine.configure_plugin("test_plugin", &test_settings("hi", "t")).unwrap();
    assert!(engine.take_property_option_changes().is_empty(), "an unchanged list is not re-sent");
    engine.configure_plugin("test_plugin", &test_settings("yo", "t")).unwrap();
    let changes = engine.take_property_option_changes();
    assert_eq!(option_set(&changes, "greetings").unwrap().options[0].value, "yo");

    // Disabling clears the options and reports the sources as empty
    engine.disable_plugin("test_plugin").unwrap();
    assert!(engine.property_options().is_empty());
    let changes = engine.take_property_option_changes();
    assert_eq!(option_set(&changes, "greetings").map(|s| s.options.len()), Some(0));
    // The stub keeps the schema (read-only in the editor)
    assert_eq!(engine.node_definition(node).unwrap().properties.len(), 2);
}

#[test]
fn option_limits_are_enforced_by_the_host() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("flood", "t")).unwrap();
    let sets = engine.property_options();
    assert_eq!(option_set(&sets, "flood").unwrap().options.len(), MAX_OPTIONS_PER_SOURCE);
    // A value longer than the cap and an empty value are dropped; the valid one stays
    let long = option_set(&sets, "long").unwrap();
    assert_eq!(long.options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(), ["ok"]);
}

#[test]
fn ha_plugin_publishes_entity_options_for_the_picker() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let state = |entity: &str, state: &str, attributes: serde_json::Value| serde_json::json!({
        "entity_id": entity, "state": state, "attributes": attributes,
        "last_changed": "2026-10-30T23:15:42.500000+02:00", "last_updated": "2026-10-30T23:15:42.500000+02:00",
        "context": {"id": "01X", "parent_id": null, "user_id": null}
    });
    *ha.entity_list.lock().unwrap() = vec![
        state("light.kitchen", "on", serde_json::json!({"friendly_name": "Kitchen light"})),
        state("binary_sensor.door", "off", serde_json::json!({"friendly_name": "Door", "device_class": "door"})),
        state("sensor.custom", "1", serde_json::json!({"icon": "mdi:cat"})),
        state("person.alice", "home", serde_json::json!({"friendly_name":"Alice"})),
        state("device_tracker.phone", "home", serde_json::json!({"friendly_name":"Phone"})),
        state("zone.home", "1", serde_json::json!({"friendly_name":"Home"})),
        state("calendar.work", "on", serde_json::json!({"friendly_name":"Work"})),
    ];
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let mut sets = Vec::new();
    for _ in 0..300 {
        engine.tick();
        sets.extend(engine.take_property_option_changes());
        if sets.iter().any(|s| s.source == "entities") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let set = sets.iter().find(|s| s.plugin == "home_assistant" && s.source == "entities").expect("the entity options are published once connected");
    let summary: Vec<_> = set.options.iter().filter(|o| !["person.","device_tracker.","zone.","calendar."].iter().any(|d|o.value.starts_with(d))).map(|o| (o.value.as_str(), o.label.as_str(), o.detail.as_str(), o.icon.as_deref().unwrap())).collect();
    assert_eq!(summary, [
        ("light.kitchen", "Kitchen light", "light.kitchen", "mdi:lightbulb"),
        ("binary_sensor.door", "Door", "binary_sensor.door", "mdi:door-closed"),
        ("sensor.custom", "sensor.custom", "sensor.custom", "mdi:cat"),
    ]);
    assert_eq!(set.options.len(),7);
    for (source,ids) in [("persons",vec!["person.alice","device_tracker.phone"]),("zones",vec!["zone.home"]),("calendars",vec!["calendar.work"])] {
        let subset = sets.iter().find(|s|s.source == source).unwrap();
        assert_eq!(subset.options.iter().map(|o|o.value.as_str()).collect::<Vec<_>>(),ids);
        assert!(subset.options.iter().all(|o|set.options.contains(o)));
    }
    let published = engine.property_options();
    assert!(published.iter().any(|s| s.source == "actions:light.kitchen" && s.options.iter().map(|o|o.value.as_str()).collect::<Vec<_>>() == ["light.turn_on"]));
    assert!(published.iter().any(|s| s.source == "action_targets" && s.options.iter().map(|o|o.value.as_str()).collect::<Vec<_>>() == ["light.kitchen"]));
    // Disabling the plugin clears the options
    engine.disable_plugin("home_assistant").unwrap();
    assert!(engine.property_options().is_empty());
    let cleared = engine.take_property_option_changes();
    assert_eq!(cleared.len(), published.len());
    assert!(cleared.iter().all(|s| s.options.is_empty()));
}

#[test]
fn ha_plugin_state_and_numeric_triggers_fire_with_typed_outputs() {
    use serde_json::json;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let state_node = NodeId::generate();
    let numeric_node = NodeId::generate();
    engine
        .create_node(
            state_node,
            "ha/trigger_state",
            &[
                ("entity_id".into(), "light.hall".into()),
                ("from".into(), "off".into()),
                ("to".into(), "on".into()),
                ("for".into(), "00:05:00".into()),
            ],
        )
        .unwrap();
    engine
        .create_node(
            numeric_node,
            "ha/trigger_numeric_state",
            &[("entity_id".into(), "sensor.temperature".into()), ("above".into(), "20".into())],
        )
        .unwrap();
    let state_sink = sink(&mut engine, state_node, 0);
    let numeric_sink = sink(&mut engine, numeric_node, 0);
    run_until(&mut engine, |_| ha.trigger_requests.lock().unwrap().len() == 2);
    let requests = ha.trigger_requests.lock().unwrap().clone();
    let state_request = requests.iter().find(|r| r["trigger"]["trigger"] == "state").unwrap();
    let numeric_request = requests.iter().find(|r| r["trigger"]["trigger"] == "numeric_state").unwrap();
    assert_eq!(state_request["trigger"], json!({"trigger":"state","entity_id":["light.hall"],"from":"off","to":"on","for":"00:05:00"}));
    assert_eq!(numeric_request["trigger"], json!({"trigger":"numeric_state","entity_id":["sensor.temperature"],"above":20.0}));
    let state = |entity: &str, value: &str| json!({"entity_id":entity,"state":value,"attributes":{},"last_changed":"2026-10-30T10:00:00Z","last_updated":"2026-10-30T10:00:00Z","context":{"id":"t"}});
    ha.fire(
        state_request["id"].as_u64().unwrap(),
        json!({"platform":"state","entity_id":"light.hall","from_state":state("light.hall","off"),"to_state":state("light.hall","on")}),
    );
    count_executions(&mut engine, state_sink, 0, 1);
    assert_eq!(output(&engine, state_node, 2), Some("light.hall".into()));
    assert_eq!(field_text(&output(&engine, state_node, 3).unwrap(), "state").as_deref(), Some("off"));
    assert_eq!(field_text(&output(&engine, state_node, 4).unwrap(), "state").as_deref(), Some("on"));
    assert_eq!(output(&engine, state_node, 4).unwrap().get_type(), DataType::Named("ha.state".into()));
    ha.fire(numeric_request["id"].as_u64().unwrap(),json!({"platform":"numeric_state","entity_id":"sensor.temperature","from_state":state("sensor.temperature","19"),"to_state":state("sensor.temperature","21.5")}));
    count_executions(&mut engine, numeric_sink, 0, 1);
    assert_eq!(output(&engine, numeric_node, 2), Some("sensor.temperature".into()));
    assert_eq!(field_text(&output(&engine, numeric_node, 4).unwrap(), "state").as_deref(), Some("21.5"));
    assert_eq!(output(&engine, numeric_node, 5), Some(DataValue::from(21.5)));
    assert_eq!(field_text(&output(&engine, numeric_node, 1).unwrap(), "platform").as_deref(), Some("numeric_state"));
    // Node resource Drop cancels the subscription, without removing the other trigger.
    engine.remove_node(state_node);
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) >= 1);
    // Clearing required configuration stops the subscription; restoring it creates a fresh one.
    engine.set_node_property(numeric_node, "entity_id", "".into()).unwrap();
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) >= 2);
    assert_eq!(ha.trigger_requests.lock().unwrap().len(), 2);
    engine.set_node_property(numeric_node, "entity_id", "sensor.other".into()).unwrap();
    run_until(&mut engine, |_| ha.trigger_requests.lock().unwrap().len() == 3);
    assert_eq!(ha.trigger_requests.lock().unwrap()[2]["trigger"]["entity_id"], json!(["sensor.other"]));
}

#[test]
fn ha_plugin_event_trigger_fires_and_rejections_log_on_the_node() {
    use serde_json::json;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let event_node = NodeId::generate();
    engine
        .create_node(
            event_node,
            "ha/trigger_event",
            &[("event_type".into(), "button_pressed".into()), ("event_data".into(), r#"{"button":"door"}"#.into())],
        )
        .unwrap();
    let event_sink = sink(&mut engine, event_node, 0);
    run_until(&mut engine, |_| ha.trigger_requests.lock().unwrap().len() == 1);
    let request = ha.trigger_requests.lock().unwrap()[0].clone();
    assert_eq!(request["type"], "subscribe_trigger");
    assert_eq!(request["trigger"], json!({"trigger":"event","event_type":["button_pressed"],"event_data":{"button":"door"}}));
    ha.fire(
        request["id"].as_u64().unwrap(),
        json!({"platform":"event","event":{"event_type":"button_pressed","data":{"button":"door","count":2}}}),
    );
    count_executions(&mut engine, event_sink, 0, 1);
    assert_eq!(output(&engine, event_node, 3), Some("button_pressed".into()));
    assert_eq!(field_text(&output(&engine, event_node, 2).unwrap(), "button").as_deref(), Some("door"));
    assert_eq!(output(&engine, event_node, 1).unwrap().get_type(), DataType::Object);
    // Local JSON errors cancel the old subscription and retain the user's text for editing.
    engine.set_node_property(event_node, "event_data", "{".into()).unwrap();
    run_until(&mut engine, |e| {
        e.host_state.logs.iter().any(|(node, message)| *node == event_node && message.contains("Invalid event data JSON"))
    });
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) >= 1);
    assert_eq!(ha.trigger_requests.lock().unwrap().len(), 1);
    engine.set_node_property(event_node, "event_data", "".into()).unwrap();
    run_until(&mut engine, |_| ha.trigger_requests.lock().unwrap().len() >= 2);
    // A server rejection arrives during plugin-tick, then poll logs it under this node's id.
    engine.set_node_property(event_node, "event_type", "reject".into()).unwrap();
    run_until(&mut engine, |e| {
        e.host_state.logs.iter().any(|(node, message)| *node == event_node && message.contains("Trigger rejected by fake HA"))
    });
}

#[test]
fn ha_plugin_invalid_trigger_json_logs_once_in_node_context_without_a_connection() {
    let Some(bytes) = plugin_bytes("ha_plugin") else { return };
    let mut engine = TickEngine::new();
    engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    engine.enable_plugin("home_assistant", &[]).unwrap();
    let node = NodeId::generate();
    engine.create_node(node, "ha/trigger_event", &[("event_type".into(), "test".into()), ("event_data".into(), "{".into())]).unwrap();
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(owner, message)| *owner == node && message.contains("Invalid event data JSON")));
    engine.host_state.logs.clear();
    for _ in 0..3 {
        engine.tick();
        assert!(!engine.host_state.logs.iter().any(|(_, message)| message.contains("Invalid event data JSON")));
    }
    engine.set_node_property(node, "event_data", "[]".into()).unwrap();
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(owner, message)| *owner == node && message.contains("Event data must be a JSON object")));
    engine.host_state.logs.clear();
    engine.set_node_property(node, "event_data", "{}".into()).unwrap();
    engine.tick();
    assert!(!engine.host_state.logs.iter().any(|(_, message)| message.contains("Event data must")));
    engine.remove_node(node);
}

#[test]
fn ha_plugin_action_form_sends_fields_target_and_response_requests() {
    use serde_json::json;
    const DONE: u32 = u32::MAX - 3;
    const FAILED: u32 = u32::MAX - 2;
    const ERROR: u32 = u32::MAX - 1;
    const RESPONSE: u32 = u32::MAX;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let action = NodeId::generate();
    engine.create_node(action,"ha/action",&[]).unwrap();
    engine.set_node_property(action,"target","light.kitchen".into()).unwrap();
    run_until(&mut engine, |e| e.property_options().iter().any(|s| s.source == "actions:light.kitchen"));
    let actions = engine.property_options().into_iter().find(|s| s.source == "actions:light.kitchen").unwrap();
    let option = &actions.options[0];
    assert_eq!((option.label.as_str(),option.detail.as_str(),option.icon.as_deref()),("Turn on","light.turn_on",Some("mdi:lightbulb")));
    engine.set_node_property(action,"action","light.turn_on".into()).unwrap();
    let def = engine.node_definition(action).unwrap();
    assert!(def.pins.iter().any(|p|p.id == PinId(2) && p.name == "Brightness" && p.required && p.pin_type == PinType::Data(DataType::Number)));
    assert_eq!(def.pins.iter().filter(|p|p.direction == PinDirection::Input).map(|p|p.name.as_str()).collect::<Vec<_>>(),["In","Entity","Brightness","effect","rgb_color","transition"]);
    assert_eq!(def.properties[0].kind,PropertyKind::Dynamic{source:"action_targets".into(),plugin:"home_assistant".into()});
    assert_eq!(def.properties[1].kind,PropertyKind::Dynamic{source:"actions:light.kitchen".into(),plugin:"home_assistant".into()});
    let done = sink(&mut engine,action,DONE);
    let failed = sink(&mut engine,action,FAILED);
    // Required fields are enforced before any request can reach HA.
    engine.trigger_node(action,PinId(0));
    engine.tick();
    assert!(ha.calls.lock().unwrap().is_empty());
    engine.set_input_value(action,PinId(2),Some(number(120))).unwrap();
    engine.trigger_node(action,PinId(0));
    count_executions(&mut engine,done,0,1);
    let calls = ha.calls.lock().unwrap().clone();
    assert_eq!(calls.len(),1);
    assert_eq!(calls[0]["service_data"],json!({"brightness":120}));
    assert_eq!(calls[0]["target"],json!({"entity_id":["light.kitchen"]}));
    assert!(calls[0].get("return_response").is_none());
    assert_eq!(output(&engine,action,ERROR),Some("".into()));
    assert!(output(&engine,action,RESPONSE).is_none());
    // Both an inline value and a connected input override the property (the connection wins).
    engine.set_input_value(action,PinId(1),Some("light.inline".into())).unwrap();
    engine.trigger_node(action,PinId(0));
    count_executions(&mut engine,done,0,1);
    assert_eq!(ha.calls.lock().unwrap()[1]["target"],json!({"entity_id":["light.inline"]}));
    constant(&mut engine,"light.connected".into(),action,1);
    engine.trigger_node(action,PinId(0));
    count_executions(&mut engine,done,0,1);
    assert_eq!(ha.calls.lock().unwrap()[2]["target"],json!({"entity_id":["light.connected"]}));
    // A response-capable service requests and exposes only its response object.
    engine.set_node_property(action,"action","weather.get_forecasts".into()).unwrap();
    engine.set_input_value(action,PinId(2),Some("daily".into())).unwrap();
    engine.trigger_node(action,PinId(0));
    count_executions(&mut engine,done,0,1);
    let calls = ha.calls.lock().unwrap().clone();
    assert_eq!(calls[3]["return_response"],true);
    assert_eq!(calls[3]["service_data"],json!({"type":"daily"}));
    let DataValue::Object { fields, .. } = output(&engine,action,RESPONSE).unwrap() else { panic!("response object") };
    assert!(fields.contains_key("weather.home"));
    engine.set_node_property(action,"action","broken.missing".into()).unwrap();
    engine.trigger_node(action,PinId(0));
    count_executions(&mut engine,failed,0,1);
    assert!(matches!(output(&engine,action,ERROR),Some(DataValue::String(e)) if e.contains("Service not found")));

    // Saved field IDs, labels, required flags and inputs survive a fresh, offline plugin instance.
    engine.set_node_property(action,"action","light.turn_on".into()).unwrap();
    engine.set_input_value(action,PinId(2),Some(number(80))).unwrap();
    let definition = engine.node_definition(action).unwrap();
    let graph = engine.export_snippet(&[action]);
    let bytes = plugin_bytes("ha_plugin").unwrap();
    let mut offline = TickEngine::new();
    offline.load_plugin(&bytes,None, |_|vec![]).unwrap();
    assert!(offline.load_graph(&graph).is_empty());
    assert_eq!(offline.node_definition(action).unwrap(),definition);
    assert_eq!(offline.export_graph().nodes[0].properties,graph.nodes[0].properties);
    offline.trigger_node(action,PinId(0));
    offline.tick();
    assert!(matches!(output(&offline,action,ERROR),Some(DataValue::String(e)) if e.contains("not configured")));
}

#[test]
fn ha_plugin_entity_emissions_remain_pullable_across_ticks() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    ha.states.lock().unwrap().insert("sensor.temperature".into(),"19.5".into());
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let entity = NodeId::generate();
    engine.create_node(entity,"ha/entity",&[("entity_id".into(),"sensor.temperature".into())]).unwrap();
    assert!(engine.node_definition(entity).unwrap().pins.iter().all(|p|p.direction == PinDirection::Output && matches!(p.pin_type,PinType::Data(_))));
    run_until(&mut engine, |e| output(e,entity,1) == Some(DataValue::from(19.5)));
    ha.change("sensor.temperature","19.5","21.5");
    run_until(&mut engine, |e| output(e,entity,1) == Some(DataValue::from(21.5)));
    for _ in 0..8 { engine.tick(); }
    let get = NodeId::generate();
    engine.create_node(get,"ha/get_state",&[("in:1".into(),"light.kitchen".into())]).unwrap();
    let probe = sink(&mut engine,get,2);
    engine.connect_data(entity,PinId(1),probe,PinId(1)).unwrap();
    engine.trigger_node(get,PinId(0));
    count_executions(&mut engine,probe,0,1);
    let trace = engine.executed.iter().find(|t|t.node_id == probe).unwrap();
    assert!(trace.inputs.contains(&(PinId(1),Some(DataValue::from(21.5)))),"{trace:?}");
    // Retargeting subscribes anew and immediately exposes the current boolean state.
    engine.set_node_property(entity,"entity_id","light.kitchen".into()).unwrap();
    run_until(&mut engine, |e| output(e,entity,2) == Some(DataValue::Bool(false)));
    assert_eq!(output(&engine, entity, 1), None, "retargeting to a boolean state clears the old number");
    ha.change("light.kitchen","off","on");
    run_until(&mut engine, |e| output(e,entity,2) == Some(DataValue::Bool(true)));
    for _ in 0..8 { engine.tick(); }
    engine.connect_data(entity,PinId(2),probe,PinId(1)).unwrap();
    engine.trigger_node(get,PinId(0));
    count_executions(&mut engine,probe,0,1);
    let trace = engine.executed.iter().find(|t|t.node_id == probe).unwrap();
    assert!(trace.inputs.contains(&(PinId(1),Some(DataValue::Bool(true)))),"{trace:?}");
    ha.change("light.kitchen", "on", "unavailable");
    run_until(&mut engine, |e| output(e, entity, 0) == Some(DataValue::from("unavailable")));
    assert_eq!(output(&engine, entity, 1), None);
    assert_eq!(output(&engine, entity, 2), None, "an unknown state clears the old boolean");
}

#[test]
fn ha_plugin_action_snapshot_restores_offline_through_wasm() {
    let Some(bytes) = plugin_bytes("ha_plugin") else { return };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let action = NodeId::generate();
    let list = |values: &[&str]| DataValue::List {
        element_type: DataType::String,
        elements: values.iter().map(|s| DataValue::from(*s)).collect(),
    };
    engine.create_node(action, "ha/action", &[
        ("action".into(), "light.turn_on".into()),
        ("target".into(), "light.kitchen".into()),
        ("fields".into(), list(&["brightness:number", "rgb_color:color"])),
        ("field_labels".into(), list(&["Brightness", "RGB color"])),
        ("required_fields".into(), list(&["brightness"])),
        ("target_domains".into(), list(&["light"])),
        ("supports_response".into(), DataValue::Bool(false)),
        ("in:2".into(), number(80)),
    ]).unwrap();
    let def = engine.node_definition(action).unwrap();
    assert_eq!(def.pins[2].id, PinId(2));
    assert_eq!(def.pins[2].name, "Brightness");
    assert!(def.pins[2].required);
    assert_eq!(def.pins[3].pin_type, PinType::Data(DataType::Color));
    assert_eq!(def.properties[0].kind, PropertyKind::Dynamic { source: "action_targets".into(), plugin: "home_assistant".into() });
    assert_eq!(def.properties[1].kind, PropertyKind::Dynamic { source: "actions:light.kitchen".into(), plugin: "home_assistant".into() });
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.node_definition(action).unwrap(), def);
    assert_eq!(restored.export_graph(), graph);
    restored.trigger_node(action, PinId(0));
    restored.tick();
    assert!(matches!(output(&restored, action, u32::MAX - 1), Some(DataValue::String(e)) if e.contains("not configured")));
    assert!(restored.executed.iter().any(|t|t.node_id == action && t.fired.contains(&PinId(u32::MAX - 2))));
}

fn condition_test_state(entity: &str, state: &str) -> serde_json::Value {
    serde_json::json!({"entity_id":entity,"state":state,"attributes":{},"last_changed":"2026-10-30T10:00:00Z","last_updated":"2026-10-30T10:00:00Z"})
}

#[test]
fn ha_plugin_state_condition_routes_using_the_live_cache() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    *ha.entity_list.lock().unwrap() = vec![condition_test_state("light.kitchen", "off")];
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let gate = NodeId::generate();
    engine.create_node(gate, "ha/condition_state", &[("entity_id".into(), "light.kitchen".into()), ("state".into(), "on".into())]).unwrap();
    let yes = sink(&mut engine, gate, 1);
    let no = sink(&mut engine, gate, 2);
    let entity = NodeId::generate();
    engine.create_node(entity, "ha/entity", &[("entity_id".into(), "light.kitchen".into())]).unwrap();
    run_until(&mut engine, |e| e.property_options().iter().any(|s| s.source == "entities" && !s.options.is_empty()));
    engine.trigger_node(gate, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(false)));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == no));
    assert!(!engine.executed.iter().any(|t| t.node_id == yes));
    ha.change("light.kitchen", "off", "on");
    run_until(&mut engine, |e| output(e, entity, 2) == Some(DataValue::Bool(true)));
    engine.trigger_node(gate, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(true)));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == yes));
    assert!(!engine.executed.iter().any(|t| t.node_id == no));
    // Unknown entities fail closed and log only once while the situation remains unchanged.
    engine.set_node_property(gate, "entity_id", "light.missing".into()).unwrap();
    for expected in [1, 0] {
        engine.host_state.logs.clear();
        engine.trigger_node(gate, PinId(0)); engine.tick();
        assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(false)));
        assert_eq!(engine.host_state.logs.iter().filter(|(_, message)| message.contains("returning No")).count(), expected);
    }
}

#[test]
fn ha_plugin_numeric_condition_uses_a_live_entity_bound() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    *ha.entity_list.lock().unwrap() = vec![condition_test_state("sensor.temp", "21"), condition_test_state("input_number.bound", "20")];
    ha.states.lock().unwrap().insert("input_number.bound".into(), "20".into());
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let gate = NodeId::generate();
    engine.create_node(gate, "ha/condition_numeric_state", &[("entity_id".into(), "sensor.temp".into()), ("above".into(), "input_number.bound".into()), ("below".into(), "22".into())]).unwrap();
    let yes = sink(&mut engine, gate, 1);
    let no = sink(&mut engine, gate, 2);
    let entity = NodeId::generate();
    engine.create_node(entity, "ha/entity", &[("entity_id".into(), "input_number.bound".into())]).unwrap();
    run_until(&mut engine, |e| e.property_options().iter().any(|s| s.source == "entities" && !s.options.is_empty()));
    engine.trigger_node(gate, PinId(0));
    count_executions(&mut engine, yes, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(true)));
    ha.change("input_number.bound", "20", "21");
    run_until(&mut engine, |e| output(e, entity, 1) == Some(DataValue::from(21.0)));
    engine.trigger_node(gate, PinId(0));
    count_executions(&mut engine, no, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(false)));
}

#[test]
fn ha_plugin_template_condition_completes_asynchronously_and_unsubscribes() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else { return };
    let gate = NodeId::generate();
    engine.create_node(gate, "ha/condition_template", &[("value_template".into(), "{{ true }}".into())]).unwrap();
    let yes = sink(&mut engine, gate, 1);
    let no = sink(&mut engine, gate, 2);
    run_until(&mut engine, |_| ha.subs.lock().unwrap().contains_key("__cache"));
    let before = ha.unsubscribed.load(Ordering::SeqCst);
    engine.trigger_node(gate, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, gate, 3), None, "rendering is asynchronous");
    count_executions(&mut engine, yes, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(true)));
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) > before);
    engine.set_node_property(gate, "value_template", "{{ false }}".into()).unwrap();
    engine.trigger_node(gate, PinId(0));
    count_executions(&mut engine, no, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(false)));
}

#[test]
fn plugin_clears_data_and_rejects_clears_of_other_pins() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let node = NodeId::generate();
    engine.create_node(node, "test/clear", &[]).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, node, 1), Some(number(42)));
    engine.trigger_node(node, PinId(2));
    engine.tick();
    assert_eq!(output(&engine, node, 1), None);
    assert!(engine.executed.iter().find(|t| t.node_id == node).unwrap().outputs.is_empty());
    let errors: Vec<_> = engine.host_state.logs.iter().filter(|(id, text)| *id == node && text.contains("not a data output")).collect();
    assert_eq!(errors.len(), 4, "input, exec output and unknown pins must be rejected: {errors:?}");
}

#[test]
fn split_plugin_struct_pins_flow_persist_and_survive_disable() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let source = NodeId::generate();
    let sink = NodeId::generate();
    engine.create_node(source, "test/point_source", &[("x".into(), number(5)), ("y".into(), number(7))]).unwrap();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine.connect_data(source, PinId(1), sink, PinId(1)).unwrap();
    assert_eq!(engine.set_pin_split(source, PinId(1), true).unwrap().data.len(), 1);
    engine.set_pin_split(sink, PinId(1), true).unwrap();
    for i in 0..3 {
        let field = split_field_pin(PinId(1), i).unwrap();
        engine.connect_data(source, field, sink, field).unwrap();
    }
    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink, PinId(0));
    // Explicit ordering: let the source's plugin output arrive before pulling the sink's inputs.
    engine.tick();
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 4), Some(number(12)));
    let trace = engine.executed.iter().find(|t| t.node_id == sink).unwrap();
    assert!(trace.inputs.iter().any(|(p, v)| *p == split_field_pin(PinId(1), 2).unwrap() && v.is_none()));
    let graph = engine.export_graph();
    let before = engine.node_definition(source).unwrap();
    engine.disable_plugin("test_plugin").unwrap();
    assert_eq!(engine.export_graph(), graph);
    assert_eq!(engine.node_definition(source).unwrap(), before);
    assert!(engine.set_pin_split(source, PinId(1), false).is_err());
    assert!(engine.load_graph(&graph).is_empty());
    assert_eq!(engine.export_graph(), graph);
    engine.enable_plugin("test_plugin", &[]).unwrap();
    assert_eq!(engine.export_graph(), graph);
    assert!(engine.load_graph(&graph).is_empty());
    let snippet = engine.export_snippet(&[source, sink]);
    let (map, warnings) = engine.import_snippet(&snippet);
    assert!(warnings.is_empty());
    let source = map.iter().find(|(old, _)| *old == source).unwrap().1;
    let sink = map.iter().find(|(old, _)| *old == sink).unwrap().1;
    engine.trigger_node(source, PinId(0));
    engine.tick();
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 4), Some(number(12)));
    assert_eq!(engine.set_pin_split(sink, PinId(1), false).unwrap().data.len(), 3);
}

#[test]
fn plugin_cost_tracks_properties_and_controls_budget() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let id = NodeId::generate();
    engine.create_node(id, "test/cost", &[("cost".into(), number(7))]).unwrap();
    assert_eq!(engine.node_definition(id).unwrap().cost, 7);
    engine.budget.limit = Some(3);
    engine.trigger_node(id, PinId(0));
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(engine.last_tick, TickStats { spent: 7, deferred: 1 });
    engine.set_node_property(id, "cost", number(2)).unwrap();
    assert_eq!(engine.node_definition(id).unwrap().cost, 2);
    engine.tick();
    assert_eq!(engine.last_tick, TickStats { spent: 2, deferred: 0 });
    let graph = engine.export_graph();
    assert!(engine.load_graph(&graph).is_empty());
    assert_eq!(engine.node_definition(id).unwrap().cost, 2);
}

#[test]
fn core_c_host_clock_and_reserved_pins_are_visible_through_the_catalog_and_execution() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("clock", "t")).unwrap();
    let sets = engine.property_options();
    assert_eq!(option_set(&sets, "clock").unwrap().options[0].value, "0:0");
    let definition = engine.registry.get_definition("test/clock").unwrap().clone();
    assert_eq!(definition.pins.iter().map(|p| p.id).collect::<Vec<_>>(), [PinId(0), PinId(1), PinId(2)]);
    let node = NodeId::generate();
    engine.create_node(node, "test/clock", &[]).unwrap();
    assert_eq!(engine.node_definition(node).unwrap().pins, definition.pins);
    engine.host_state.tick_duration = std::time::Duration::from_millis(37);
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, node, 1), Some(number(1)));
    assert_eq!(output(&engine, node, 2), Some(number(37)));
    let trace = engine.executed.iter().find(|t| t.node_id == node).unwrap();
    assert_eq!(trace.outputs, [(PinId(1), number(1)), (PinId(2), number(37))]);
}

#[test]
fn core_c_editing_existing_plugin_properties_survives_instance_recreation() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let node = NodeId::generate();
    engine.create_node(node, "test/point_source", &[("x".into(), number(2)), ("y".into(), number(3))]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(id, log)| *id == crash && log.contains("Plugin crashed")));
    // Edit before the next tick recreates the resource from the stored properties.
    engine.set_node_property(node, "y", number(9)).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    let Some(DataValue::Object { fields, .. }) = output(&engine, node, 1) else { panic!("missing point") };
    assert_eq!(fields["x"].value, DataValue::from(2.0));
    assert_eq!(fields["y"].value, DataValue::from(9.0));
    let properties = engine.node_model(node).unwrap().properties;
    assert_eq!(properties.iter().filter(|(key, _)| key == "y").count(), 1);
    assert!(properties.contains(&("y".into(), DataValue::from(9.0))));
}

#[test]
fn delay_queue_timers_and_sdk_time_rounding() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else { return };
    let node = NodeId::generate();
    engine.create_node(node, "delay/queue", &[]).unwrap();
    engine.set_input_value(node, PinId(2), Some(DataValue::Ticks(5))).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    engine.set_input_value(node, PinId(2), Some(DataValue::Duration(std::time::Duration::from_millis(100) + std::time::Duration::from_nanos(1)))).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    // The second task rounds up to two ticks, but the older, longer task supplies progress.
    assert_eq!(engine.delay_timers(), vec![NodeTimer { node, total_ms: 500, remaining_ms: 400, count: 2 }]);
    engine.tick();
    assert_eq!(engine.delay_timers()[0].count, 2);
    engine.tick();
    assert_eq!(engine.delay_timers()[0].count, 1);
    engine.tick();
    engine.tick();
    assert!(engine.delay_timers().is_empty());
    engine.set_input_value(node, PinId(2), Some(DataValue::Duration(std::time::Duration::from_nanos(1)))).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(engine.delay_timers()[0].total_ms, 100);
    engine.tick();
    assert!(engine.delay_timers().is_empty());
}
