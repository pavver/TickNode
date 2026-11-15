#[test]
fn pure_plugin_node_pull_cache_and_recompute() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };

    let pure_id = NodeId::generate();
    engine.create_node(pure_id, "test/pure_double", &[]).unwrap();
    assert_eq!(engine.is_node_pure(pure_id), Some(true));

    // Provide initial input (21) via inline input value
    engine
        .set_input_value(pure_id, PinId(1), Some(number(21)))
        .unwrap();

    // Two active debug consumers chained together
    let debug1 = NodeId::generate();
    let debug2 = NodeId::generate();
    engine.create_node(debug1, "test/debug", &[]).unwrap();
    engine.create_node(debug2, "test/debug", &[]).unwrap();

    // Wire pure node output (0) to both debug nodes' Value inputs (1)
    engine.connect_data(pure_id, PinId(0), debug1, PinId(1)).unwrap();
    engine.connect_data(pure_id, PinId(0), debug2, PinId(1)).unwrap();

    // Trigger both debug nodes in the same tick
    engine.trigger_node(debug1, PinId(0));
    engine.trigger_node(debug2, PinId(0));
    engine.tick();

    // Both debug nodes got the doubled value
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, msg)| *n == debug1 && msg == "42")
    );
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, msg)| *n == debug2 && msg == "42")
    );

    // execute ran at most once per tick (cached in pure_cache)
    let cached = engine.pure_cache.get(&pure_id).expect("cached pure outputs");
    assert_eq!(cached.get(&PinId(2)), Some(&number(1)));

    // Second tick: change input to 50
    engine.host_state.logs.clear();
    engine
        .set_input_value(pure_id, PinId(1), Some(number(50)))
        .unwrap();

    engine.trigger_node(debug1, PinId(0));
    engine.trigger_node(debug2, PinId(0));
    engine.tick();

    // Value recomputed after input change
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, msg)| *n == debug1 && msg == "100")
    );
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, msg)| *n == debug2 && msg == "100")
    );
    let cached = engine.pure_cache.get(&pure_id).expect("cached pure outputs");
    assert_eq!(cached.get(&PinId(2)), Some(&number(2)));
}

#[test]
fn invalid_pure_definition_loads_as_non_pure_and_stub_is_not_pure() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };

    // test/pure_invalid has pure: true but contains an exec pin: must be treated as non-pure
    let invalid_id = NodeId::generate();
    engine
        .create_node(invalid_id, "test/pure_invalid", &[])
        .unwrap();
    assert_eq!(
        engine.is_node_pure(invalid_id),
        Some(false),
        "invalid pure node with exec pin must be non-pure"
    );

    // Valid pure node: check is_pure() is true
    let pure_id = NodeId::generate();
    engine.create_node(pure_id, "test/pure_double", &[]).unwrap();
    assert_eq!(engine.is_node_pure(pure_id), Some(true));

    // Disabling the plugin turns instances into stubs; stub is_pure() must be false
    engine.disable_plugin("test_plugin").unwrap();
    assert_eq!(
        engine.is_node_pure(pure_id),
        Some(false),
        "stub of a pure plugin node must stay non-pure"
    );
}

#[test]
fn pure_node_execution_output_is_ignored_with_warning() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };

    let pure_id = NodeId::generate();
    engine
        .create_node(pure_id, "test/pure_exec_pulse", &[])
        .unwrap();

    let debug = NodeId::generate();
    engine.create_node(debug, "test/debug", &[]).unwrap();
    engine.connect_data(pure_id, PinId(0), debug, PinId(1)).unwrap();

    // Trigger debug to pull from pure_exec_pulse
    engine.trigger_node(debug, PinId(0));
    engine.tick();

    // Data output is in pure cache and pulled
    assert_eq!(
        engine
            .pure_cache
            .get(&pure_id)
            .and_then(|m| m.get(&PinId(0))),
        Some(&number(42))
    );

    // Warning was logged
    let warning_count = engine
        .host_state
        .logs
        .iter()
        .filter(|(n, msg)| *n == pure_id && msg.contains("[WARN]"))
        .count();
    assert_eq!(warning_count, 1, "warning logged once");

    // Second pull without state change: should not repeat the warning
    engine.trigger_node(debug, PinId(0));
    engine.tick();
    let warning_count2 = engine
        .host_state
        .logs
        .iter()
        .filter(|(n, msg)| *n == pure_id && msg.contains("[WARN]"))
        .count();
    assert_eq!(warning_count2, 1, "warning was throttled (not repeated)");
}
