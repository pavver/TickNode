use super::*;

#[test]
fn disabled_nodes_are_excluded_from_sources_and_idle_tick() {
    let mut engine = TickEngine::new();
    let clock = NodeId::generate();
    engine.create_node(clock, "clock", &[]).unwrap();
    assert!(engine.sources.contains(&clock));
    assert!(!engine.is_disabled(clock));

    engine.set_disabled([clock], true);
    assert!(engine.is_disabled(clock));
    assert!(
        !engine.sources.contains(&clock),
        "disabled node must not be in sources"
    );

    // Idle tick does not visit the disabled clock
    engine.tick();
    assert_eq!(engine.last_tick.spent, 0);
    assert!(engine.executed.is_empty());

    // Re-enabling puts it back in sources
    engine.set_disabled([clock], false);
    assert!(!engine.is_disabled(clock));
    assert!(engine.sources.contains(&clock));
}

#[test]
fn disabled_nodes_skip_queue_execution_and_pure_pulls() {
    let mut engine = TickEngine::new();
    let c = NodeId::generate();
    let add = NodeId::generate();
    engine
        .create_node(c, "const", &[("value".into(), 10i64.into())])
        .unwrap();
    engine.create_node(add, "math/add", &[]).unwrap();
    engine.connect_data(c, PinId(0), add, PinId(0)).unwrap();

    // Normal pure pull
    assert_eq!(
        engine.pull_data(c, PinId(0), &mut vec![]),
        Some(10i64.into())
    );

    // When disabled, pure pull through it yields None
    engine.set_disabled([c], true);
    assert_eq!(engine.pull_data(c, PinId(0), &mut vec![]), None);

    // Triggered node in execution queue is skipped
    engine.trigger_node(c, PinId(0));
    engine.tick();
    assert!(
        engine.executed.is_empty(),
        "disabled node should not execute"
    );

    // Re-enabling allows pure pull again
    engine.set_disabled([c], false);
    assert_eq!(
        engine.pull_data(c, PinId(0), &mut vec![]),
        Some(10i64.into())
    );
}

#[test]
fn removing_node_clears_disabled_flag() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "const", &[]).unwrap();
    engine.set_disabled([id], true);
    assert!(engine.is_disabled(id));

    engine.remove_node(id);
    assert!(!engine.is_disabled(id));
}

#[test]
fn disabled_nodes_drop_finished_tasks_without_error_spam() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "sleep", &[]).unwrap();
    engine.host_state.start_delay(id, 1);
    engine.set_disabled([id], true);

    // Advance clock so delay completes on tick
    engine.host_state.current_tick += 1;
    engine.tick();
    assert!(engine.executed.is_empty());
    assert!(
        engine.host_state.logs.is_empty(),
        "dropped task on disabled node must not produce error spam"
    );
}
