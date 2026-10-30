//! Інтеграційні тести WASM-плагінів. Потрібні зібрані компоненти:
//! `cargo build --target wasm32-wasip2 -p delay-plugin -p string-plugin -p math-plugin -p http-weather-plugin -p test-plugin`.
//! Якщо файлу немає, тест пропускається з повідомленням (а з `TICKNODE_REQUIRE_PLUGINS=1` — падає).
//!
//! Тести не асинхронні навмисно: синхронний wasmtime-wasi не можна викликати всередині tokio-задачі,
//! тому рушій, як і на сервері, працює поза runtime, а runtime передається для HTTP.

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
            eprintln!("{} не знайдено — тест пропущено", path.display());
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

    // Повторне завантаження не може перехопити вже зареєстровані типи.
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
    // 500 мс при такті 100 мс — 5 тактів.
    constant(&mut engine, number(500), delay, 2);
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
    constant(&mut engine, number(300), ignore, 12);
    let reset = NodeId::generate();
    engine.create_node(reset, "delay/reset", &[]).unwrap();
    constant(&mut engine, number(300), reset, 22);

    // Повторні імпульси: Ignore тримає одну задачу, Reset замінює свою.
    for _ in 0..3 {
        engine.trigger_node(ignore, PinId(10));
        engine.trigger_node(reset, PinId(20));
        engine.tick();
    }
    let owners: Vec<_> = engine.host_state.active_tasks.values().map(|t| t.node_id).collect();
    assert_eq!(owners.iter().filter(|n| **n == ignore).count(), 1);
    assert_eq!(owners.iter().filter(|n| **n == reset).count(), 1);
}

#[test]
fn removing_plugin_node_cancels_tasks() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else { return };
    let delay = NodeId::generate();
    engine.create_node(delay, "delay/queue", &[]).unwrap();
    constant(&mut engine, number(1000), delay, 2);
    engine.trigger_node(delay, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);
    engine.remove_node(delay);
    assert!(engine.host_state.active_tasks.is_empty());
}

/// Одноразовий HTTP-сервер, що відповідає `body`. Приймач отримує рядок запиту (`GET /… HTTP/1.1`).
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
    panic!("умова не виконалась за 200 тактів");
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
    // Місто кодується в URL і не додає параметрів запиту
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

    // Перенаправлення на хост без дозволу (localhost) — нода отримує 3xx, а не відповідь цілі
    let (target, target_requests) = serve_once("1,Hacked");
    let (port, _) = serve_raw(format!("HTTP/1.1 302 Found\r\nLocation: http://localhost:{target}/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
    let weather = NodeId::generate();
    engine.create_node(weather, "http/weather", &[("url".into(), DataValue::String(format!("http://127.0.0.1:{port}/{{city}}")))]).unwrap();
    let failure = sink(&mut engine, weather, 3);

    engine.trigger_node(weather, PinId(0));
    run_until(&mut engine, |e| e.execution_queue.contains(&(failure, PinId(0))));
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == weather && m.contains("HTTP 302")), "{:?}", engine.host_state.logs);
    assert!(target_requests.try_recv().is_err(), "хост пішов за перенаправленням");
}

#[test]
fn http_outside_granted_domains_is_denied() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    // Право лише на example.com — запит на 127.0.0.1 відхиляється одразу.
    let Some(mut engine) = engine_with("http_weather_plugin", vec![Capability::Http { allowed_domains: vec!["example.com".into()] }]) else { return };
    engine.host_state.runtime = Some(runtime.handle().clone());

    let weather = NodeId::generate();
    engine.create_node(weather, "http/weather", &[("url".into(), DataValue::String("http://127.0.0.1:9/{city}".into()))]).unwrap();
    let failure = sink(&mut engine, weather, 3);
    engine.trigger_node(weather, PinId(0));
    engine.tick();

    assert!(engine.host_state.active_tasks.is_empty());
    assert!(engine.execution_queue.contains(&(failure, PinId(0))));
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == weather && m.contains("не має права")));
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

    // Аварія не валить рушій: помилка йде в консоль ноди.
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("Аварія плагіна") && m.contains(reason)), "{:?}", engine.host_state.logs);

    // Екземпляр перестворено: лічильник відтворено з властивостей (крок 5), стан почався заново.
    engine.trigger_node(counter, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, counter, 1), Some(number(5)));

    // Нода-винуватець зупинена, доки її не переналаштують.
    engine.host_state.logs.clear();
    engine.trigger_node(crash, PinId(trigger_pin));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("зупинена")));
}

#[test]
fn removing_node_after_crash_does_not_touch_new_instance() {
    let Some((mut engine, counter, crash)) = crash_setup() else { return };
    // Обидві ноди мають ресурси в першому екземплярі
    engine.trigger_node(counter, PinId(0));
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    engine.tick();

    // Екземпляр мертвий; видалення ноди не має ні створювати новий, ні передавати йому старий ресурс
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
    // `poll` ticker'а рахує виклики; counter не опитується (його `poll` панікує)
    assert_eq!(output(&engine, ticker, 1), Some(number(3)));
    assert!(engine.host_state.logs.is_empty(), "{:?}", engine.host_state.logs);
}

#[test]
fn plugin_panic_restarts_instance() {
    assert_recovers(0, "паніка");
}

#[test]
fn infinite_loop_is_stopped_by_fuel() {
    assert_recovers(1, "ліміт обчислень");
}

#[test]
fn storage_is_isolated_plugin_directory() {
    let Some(bytes) = plugin_bytes("test_plugin") else { return };
    let root = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, Some(root.path()), |m| m.requested_capabilities.clone()).unwrap();

    let storage = NodeId::generate();
    engine.create_node(storage, "test/storage", &[]).unwrap();
    constant(&mut engine, DataValue::String("привіт".into()), storage, 1);
    engine.trigger_node(storage, PinId(0));
    engine.tick();

    assert_eq!(output(&engine, storage, 2), Some(DataValue::String("привіт".into())));
    let file = root.path().join("test_plugin/note.txt");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "привіт");
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

    let Some(DataValue::String(result)) = output(&engine, storage, 2) else { panic!("немає результату") };
    assert!(result.starts_with("error:"), "{result}");
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

// --- Власні типи даних ---

fn point_type() -> DataType {
    DataType::Named("test.point".into())
}

fn mode_type() -> DataType {
    DataType::Named("test.mode".into())
}

#[test]
fn plugin_types_are_registered_with_make_and_break_nodes() {
    let Some(engine) = engine_with("test_plugin", vec![]) else { return };
    let point = engine.types.get("test.point").expect("тип зареєстровано");
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
    // Для переліку Make/Break не створюються
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

    // Пін ноди з невідомим типом
    let node = NodeDefinition {
        type_name: "other/node".into(),
        title: "N".into(),
        category: "Test".into(),
        description: String::new(),
        help: String::new(),
        pins: vec![PinDefinition { id: PinId(0), name: "P".into(), pin_type: PinType::Data(DataType::Named("nope.type".into())), direction: PinDirection::Input, required: false }],
    };
    let err = engine.register_types(&[], &[node]).unwrap_err();
    assert!(err.contains("nope.type"), "{err}");

    // Поле може посилатись на тип, оголошений раніше
    let nested = TypeDefinition::new_struct("other.nested", "Nested", vec![FieldDefinition::new("p", "P", point_type())]);
    engine.register_types(&[nested], &[]).unwrap();
    assert!(engine.registry.get_definition("type/make/other.nested").is_some());
    assert_eq!(engine.registry.get_definitions().len(), before + 2);

    // Невдала спроба нічого не лишила
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

    // Named -> Named (та сама), Named -> Any, Named -> Object
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine.connect_data(make, PinId(0), debug, PinId(1)).unwrap();
    engine.connect_data(make, PinId(0), get_field, PinId(0)).unwrap();
    // Object -> Named відхиляється (строга типізація)
    assert!(matches!(engine.connect_data(object, PinId(0), brk, PinId(0)), Err(EngineError::TypeMismatch(..))));
    assert!(matches!(engine.connect_data(object, PinId(0), sink, PinId(1)), Err(EngineError::TypeMismatch(..))));
    // Інший Named тип і рядок -> перелік
    assert!(engine.connect_data(make, PinId(0), sink, PinId(2)).is_err());
    let text = NodeId::generate();
    engine.create_node(text, "const", &[("value".into(), DataValue::String("auto".into()))]).unwrap();
    assert!(engine.connect_data(text, PinId(0), sink, PinId(2)).is_err());
    // Number -> Named
    let num = NodeId::generate();
    engine.create_node(num, "const", &[("value".into(), number(1))]).unwrap();
    assert!(engine.connect_data(num, PinId(0), sink, PinId(1)).is_err());
}

#[test]
fn enum_pin_to_string_is_allowed() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    // Break не має виходу-переліку, тож перевіряємо правило напряму через реєстр типів
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
    // y — значення прямо на ноді, label — необов'язкове, але задане
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

    // Значення теговане
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

    // Break обчислюється на запит; label у точки немає
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(brk, PinId(3), probe, PinId(1)).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).expect("debug виконався");
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);
    let trace = engine.executed.iter().find(|t| t.node_id == brk).expect("break обчислено");
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
    // Структуру на ноді не задати
    assert!(engine.set_input_value(sink, PinId(1), Some(DataValue::String("x".into()))).is_err());
    // Значення пережило відхилені спроби
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

    // Через JSON, як на диску
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

    // Без плагіна нода типу не губиться: заглушка зберігає тип, властивості й з'єднання
    let mut bare = TickEngine::new();
    assert!(bare.load_graph(&graph).is_empty());
    assert_eq!(bare.export_graph(), graph);
    assert!(bare.node_unavailable(source).is_some());
}

// --- Обов'язкові входи ---

#[test]
fn make_requires_its_required_fields() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let make = NodeId::generate();
    engine.create_node(make, "type/make/test.point", &[]).unwrap();
    let def = engine.node_definition(make).unwrap();
    let required: Vec<_> = def.pins.iter().filter(|p| p.required).map(|p| p.id).collect();
    assert_eq!(required, [PinId(1), PinId(2)], "x і y обов'язкові, label — ні");

    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(make, PinId(0), probe, PinId(1)).unwrap();
    engine.set_input_value(make, PinId(1), Some(number(1))).unwrap();

    // Не задано y: значення немає, у лозі одна помилка (не щотакту)
    for _ in 0..3 {
        engine.trigger_node(probe, PinId(0));
        engine.tick();
    }
    let errors: Vec<_> = engine.host_state.logs.iter().filter(|(n, m)| *n == make && m.contains("обов'язковий вхід")).collect();
    assert_eq!(errors.len(), 1, "{:?}", engine.host_state.logs);
    assert!(errors[0].1.contains("«Y»"));
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);

    // Усе задано: тегований об'єкт без необов'язкового поля
    engine.set_input_value(make, PinId(2), Some(number(2))).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    let Some((_, Some(DataValue::Object { fields, type_name }))) = trace.inputs.first() else { panic!("{:?}", trace.inputs) };
    assert_eq!(type_name.as_deref(), Some("test.point"));
    assert_eq!(fields.keys().map(String::as_str).collect::<Vec<_>>(), ["x", "y"]);

    // Необов'язкове поле, якщо задане, з'являється
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
    engine.create_node(source, "test/counter", &[]).unwrap(); // Count ще не мав значення
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
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == sink && m.contains("«Point»")), "{:?}", engine.host_state.logs);
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

    // Чесний вихід того ж типу проходить і несе тег
    let good = NodeId::generate();
    engine.create_node(good, "test/point_source", &[]).unwrap();
    engine.trigger_node(good, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, good, 1).map(|v| v.get_type()), Some(point_type()));
}

// --- Налаштування плагіна (configure) ---

fn test_settings(greeting: &str, token: &str) -> Vec<(String, DataValue)> {
    vec![
        ("greeting".into(), DataValue::String(greeting.into())),
        ("token".into(), DataValue::String(token.into())),
    ]
}

/// Рушій з вимкненим (завантаженим, але не ввімкненим) test_plugin.
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
    assert_eq!(keys, [("greeting", false), ("token", true), ("mode", false)]);
    assert_eq!(manifest.settings[0].default, Some(DataValue::String("Hello".into())));
    assert!(matches!(&manifest.settings[2].kind, SettingKind::Select(o) if o.len() == 2));
    // Додано, але не ввімкнено: типи ще не зареєстровано
    assert!(!engine.plugin_active("test_plugin"));
    assert!(engine.registry.get_definition("test/greeter").is_none());
}

#[test]
fn configure_is_called_and_its_error_is_reported() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    let manifest = engine.plugin_manifest("test_plugin").unwrap().clone();

    // Увімкнення без токена: configure відхиляє, але плагін працює
    let defaults = effective_settings(&manifest, &[]);
    engine.enable_plugin("test_plugin", &defaults).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin").as_deref(), Some("token не задано"));
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Hello|quiet|no-token".into())));

    // Нові налаштування застосовуються без перезапуску
    engine.configure_plugin("test_plugin", &effective_settings(&manifest, &test_settings("Привіт", "good"))).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Привіт|quiet|token".into())));

    // Відповідь плагіна повертається як помилка й запам'ятовується
    let err = engine.configure_plugin("test_plugin", &test_settings("x", "bad")).unwrap_err();
    assert_eq!(err, "невірний токен");
    assert_eq!(engine.plugin_settings_error("test_plugin").as_deref(), Some("невірний токен"));
    assert!(engine.plugin_active("test_plugin"));

    // Виправлення знімає помилку
    engine.configure_plugin("test_plugin", &test_settings("x", "ok")).unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
}

#[test]
fn settings_survive_plugin_crash() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    engine.enable_plugin("test_plugin", &test_settings("Привіт", "good")).unwrap();
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Привіт|quiet|token".into())));

    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == crash && m.contains("Аварія плагіна")));

    // Екземпляр перестворено: configure викликано повторно до відтворення ноди
    assert_eq!(greet(&mut engine, greeter), Some(DataValue::String("Привіт|quiet|token".into())));
}

// --- Вимкнення й увімкнення плагіна ---

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
    // Типи знято з реєстрів
    assert!(engine.registry.get_definition("test/point_source").is_none());
    assert!(engine.registry.get_definition("type/make/test.point").is_none());
    assert!(engine.types.get("test.point").is_none());
    assert!(!engine.plugin_active("test_plugin"));
    // Граф ідентичний: типи, властивості (включно з in:*) і з'єднання
    assert_eq!(engine.export_graph(), before);
    // Заглушка: пояснення, жодних змін, не виконується
    assert_eq!(engine.node_unavailable(source).as_deref(), Some("плагін test_plugin недоступний"));
    assert!(engine.node_unavailable(debug).is_none());
    assert!(matches!(engine.set_node_property(source, "x", number(9)), Err(EngineError::PluginUnavailable(_))));
    assert!(matches!(engine.set_input_value(sink_node, PinId(2), None), Err(EngineError::PluginUnavailable(_))));
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(n, m)| *n == source && m.contains("плагін test_plugin недоступний")));
    // Дефініція стала невідомою для палітри, а нода ще має піни
    assert_eq!(engine.node_definition(source).unwrap().pins.len(), 2);

    // Повторне вимкнення — помилка; граф, збережений у вимкненому стані, теж не втрачається
    assert!(engine.disable_plugin("test_plugin").is_err());
    let json = serde_json::to_string(&engine.export_graph()).unwrap();
    let mut reloaded = TickEngine::new();
    assert!(reloaded.load_graph(&serde_json::from_str(&json).unwrap()).is_empty());
    assert_eq!(reloaded.export_graph(), before);

    // Увімкнення: заглушки знову справжні, з'єднання на місці
    let change = engine.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();
    assert_eq!(change.replaced.len(), 4);
    assert!(change.removed.data.is_empty() && change.removed.exec.is_empty());
    assert!(engine.node_unavailable(source).is_none());
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink_node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink_node, 3), Some(DataValue::String("manual:2,3".into())));

    // Перезавантажений зі заглушками граф теж оживає, коли плагін з'являється
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
fn enabling_drops_connections_that_became_invalid() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else { return };
    let counter = NodeId::generate();
    let debug = NodeId::generate();
    let ghost_link = ConnectionModel { src_node: counter, src_pin: PinId(9), dst_node: debug, dst_pin: PinId(0) };
    let graph = GraphModel {
        nodes: vec![
            NodeModel { id: counter, type_name: "test/counter".into(), properties: vec![("step".into(), number(2))], position: Position::default() },
            NodeModel { id: debug, type_name: "test/debug".into(), properties: vec![("prefix".into(), DataValue::String(String::new()))], position: Position::default() },
        ],
        // Пін 9 заглушка вивела з з'єднання, а в справжнього лічильника такого піна немає
        exec_connections: vec![ghost_link],
        data_connections: vec![ConnectionModel { src_node: counter, src_pin: PinId(1), dst_node: debug, dst_pin: PinId(1) }],
    };
    let mut graph = graph;
    graph.nodes.sort_by_key(|n| n.id.0);
    assert!(engine.load_graph(&graph).is_empty());
    assert_eq!(engine.export_graph(), graph);

    let change = engine.enable_plugin("test_plugin", &test_settings("Hi", "good")).unwrap();
    assert_eq!(change.replaced, vec![counter]);
    assert_eq!(change.removed.exec, vec![ghost_link]);
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
            NodeModel { id: a, type_name: "ghost/source".into(), properties: vec![("k".into(), number(1)), ("in:3".into(), DataValue::String("x".into()))], position: Position { x: 5.0, y: 6.0 } },
            NodeModel { id: b, type_name: "test/debug".into(), properties: vec![("prefix".into(), DataValue::String(String::new()))], position: Position::default() },
            NodeModel { id: c, type_name: "ghost/sink".into(), properties: vec![], position: Position::default() },
        ],
        exec_connections: vec![ConnectionModel { src_node: a, src_pin: PinId(0), dst_node: c, dst_pin: PinId(4) }],
        data_connections: vec![
            ConnectionModel { src_node: a, src_pin: PinId(1), dst_node: b, dst_pin: PinId(1) },
            ConnectionModel { src_node: a, src_pin: PinId(2), dst_node: c, dst_pin: PinId(5) },
        ],
    };
    let mut engine = TickEngine::new();
    assert!(engine.load_graph(&graph).is_empty());
    let mut expected = graph.clone();
    expected.nodes.iter_mut().for_each(|n| n.position = Position::default());
    assert_eq!(engine.export_graph().nodes.len(), 3);
    let mut exported = engine.export_graph();
    exported.nodes.sort_by_key(|n| n.id.0);
    expected.nodes.sort_by_key(|n| n.id.0);
    assert_eq!(exported.nodes, expected.nodes);
    assert_eq!(exported.exec_connections, graph.exec_connections);
    let mut want = graph.data_connections.clone();
    want.sort_by_key(|c| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0));
    assert_eq!(exported.data_connections, want);
    // Піни відновлено з з'єднань; тип піна дав інший кінець (Debug.Value — Any)
    let pins = engine.node_definition(a).unwrap().pins;
    assert_eq!(pins.len(), 3);
    assert!(pins.iter().any(|p| p.id == PinId(0) && p.direction == PinDirection::Output && p.pin_type == PinType::Exec));
}
