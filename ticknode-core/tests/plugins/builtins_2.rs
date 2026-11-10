#[test]
fn only_polling_plugin_nodes_are_polled() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let ticker = NodeId::generate();
    engine.create_node(ticker, "test/ticker", &[]).unwrap();
    let counter = NodeId::generate();
    engine.create_node(counter, "test/counter", &[]).unwrap();
    for _ in 0..3 {
        engine.tick();
    }
    // Ticker's `poll` counts calls; counter is not polled (its `poll` panics)
    assert_eq!(output(&engine, ticker, 1), Some(number(3)));
    assert!(
        engine.host_state.logs.is_empty(),
        "{:?}",
        engine.host_state.logs
    );
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
    let Some(bytes) = plugin_bytes("test_plugin") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine
        .load_plugin(&bytes, Some(root.path()), |m| {
            m.requested_capabilities.clone()
        })
        .unwrap();

    let storage = NodeId::generate();
    engine.create_node(storage, "test/storage", &[]).unwrap();
    constant(&mut engine, DataValue::String("hello".into()), storage, 1);
    engine.trigger_node(storage, PinId(0));
    engine.tick();

    assert_eq!(
        output(&engine, storage, 2),
        Some(DataValue::String("hello".into()))
    );
    let file = root.path().join("test_plugin/note.txt");
    assert_eq!(std::fs::read_to_string(file).unwrap(), "hello");
}

#[test]
fn storage_without_grant_is_unavailable() {
    let Some(bytes) = plugin_bytes("test_plugin") else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine
        .load_plugin(&bytes, Some(root.path()), |_| vec![])
        .unwrap();

    let storage = NodeId::generate();
    engine.create_node(storage, "test/storage", &[]).unwrap();
    engine.trigger_node(storage, PinId(0));
    engine.tick();

    let Some(DataValue::String(result)) = output(&engine, storage, 2) else {
        panic!("no result")
    };
    assert!(result.starts_with("error:"), "{result}");
    assert!(std::fs::read_dir(root.path()).unwrap().next().is_none());
}

