use super::*;
use ticknode_shared::{DataValue, PinId};

fn trace(node_id: NodeId, tick: u64, value: i64, fired: &[u32]) -> NodeTrace {
    NodeTrace {
        node_id,
        tick,
        inputs: vec![(PinId(1), Some(DataValue::from(value)))],
        outputs: vec![],
        fired: fired.iter().map(|p| PinId(*p)).collect(),
    }
}

#[test]
fn traces_of_one_interval_merge_per_node() {
    let start = Instant::now();
    let mut telemetry = Telemetry::new(start + INTERVAL);
    let (a, b) = (NodeId::generate(), NodeId::generate());
    telemetry.add_traces(&[trace(a, 1, 1, &[1])]);
    telemetry.add_traces(&[trace(b, 2, 5, &[]), trace(a, 2, 2, &[2, 1])]);
    assert!(telemetry.flush(start, &mut ticknode_core::TickEngine::new(), &HashMap::new()).is_empty(), "nothing is sent before the interval passes");

    let messages = telemetry.flush(start + INTERVAL, &mut ticknode_core::TickEngine::new(), &HashMap::new());
    let [(_, ServerMessage::Executed(traces))] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(traces.iter().map(|t| t.node_id).collect::<Vec<_>>(), [a, b]);
    assert_eq!(traces[0].tick, 2);
    assert_eq!(traces[0].inputs, vec![(PinId(1), Some(DataValue::from(2)))]);
    assert_eq!(traces[0].fired, vec![PinId(1), PinId(2)]);

    telemetry.add_traces(&[trace(a, 3, 3, &[])]);
    assert!(telemetry.flush(start + INTERVAL + INTERVAL / 2, &mut ticknode_core::TickEngine::new(), &HashMap::new()).is_empty());
    let messages = telemetry.flush(start + INTERVAL * 2, &mut ticknode_core::TickEngine::new(), &HashMap::new());
    let [(_, ServerMessage::Executed(traces))] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(traces.len(), 1, "a new interval starts with fresh traces");
    assert_eq!(traces[0].fired, Vec::<PinId>::new());
}

#[test]
fn logs_keep_order_and_report_overflow() {
    let now = Instant::now();
    let mut telemetry = Telemetry::new(now);
    let node = NodeId::generate();
    telemetry.add_logs((0..MAX_LOGS + 5).map(|i| (node, format!("line {i}"))).collect());
    telemetry.add_logs(vec![(node, "late".into())]);
    let messages = telemetry.flush(now, &mut ticknode_core::TickEngine::new(), &HashMap::new());
    assert_eq!(messages.len(), MAX_LOGS + 1);
    assert!(matches!(&messages[0].1, ServerMessage::Log { message, .. } if message == "line 0"));
    assert!(matches!(&messages[MAX_LOGS - 1].1, ServerMessage::Log { message, .. } if *message == format!("line {}", MAX_LOGS - 1)));
    assert!(matches!(&messages[MAX_LOGS].1, ServerMessage::Log { node_id, message } if node_id.0.is_nil() && message.starts_with("[WARN] 6 log lines dropped")));
    assert!(telemetry.flush(now + INTERVAL, &mut ticknode_core::TickEngine::new(), &HashMap::new()).is_empty(), "the overflow counter resets");
}

#[test]
fn discard_drops_everything_collected() {
    let now = Instant::now();
    let mut telemetry = Telemetry::new(now);
    telemetry.add_traces(&[trace(NodeId::generate(), 1, 1, &[])]);
    telemetry.add_logs(vec![(NodeId::generate(), "x".into())]);
    telemetry.discard();
    assert!(telemetry.flush(now, &mut ticknode_core::TickEngine::new(), &HashMap::new()).is_empty());
}

#[test]
fn timers_emit_only_when_first_task_or_count_changes() {
    let now = Instant::now();
    let mut telemetry = Telemetry::new(now);
    let mut engine = ticknode_core::TickEngine::new();
    let node = NodeId::generate();
    assert!(telemetry.flush(now, &mut engine, &HashMap::new()).is_empty());
    let first = engine.host_state.start_delay(node, 10);
    assert!(telemetry.flush(now + INTERVAL / 2, &mut engine, &HashMap::new()).is_empty());
    let messages = telemetry.flush(now + INTERVAL, &mut engine, &HashMap::new());
    assert_eq!(messages, vec![(None, ServerMessage::Timers(engine.delay_timers()))]);
    telemetry.discard();
    assert!(telemetry.flush(now + INTERVAL + INTERVAL / 2, &mut engine, &HashMap::new()).is_empty());
    engine.host_state.current_tick = 2;
    assert!(telemetry.flush(now + INTERVAL * 2, &mut engine, &HashMap::new()).is_empty(), "remaining time alone does not emit");
    // A second task changes count while the first timer remains selected.
    engine.host_state.start_delay(node, 1);
    let messages = telemetry.flush(now + INTERVAL * 3, &mut engine, &HashMap::new());
    let [(_, ServerMessage::Timers(timers))] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!((timers[0].total_ms, timers[0].remaining_ms, timers[0].count), (1000, 800, 2));
    engine.host_state.cancel_task(node, first);
    assert_eq!(telemetry.flush(now + INTERVAL * 4, &mut engine, &HashMap::new()), vec![(None, ServerMessage::Timers(engine.delay_timers()))]);
    // Reset in the same tick keeps count/total unchanged but changes the first task ID.
    engine.host_state.cancel_node_tasks(node);
    engine.host_state.start_delay(node, 1);
    assert_eq!(telemetry.flush(now + INTERVAL * 5, &mut engine, &HashMap::new()), vec![(None, ServerMessage::Timers(engine.delay_timers()))]);
    engine.host_state.cancel_node_tasks(node);
    assert_eq!(telemetry.flush(now + INTERVAL * 6, &mut engine, &HashMap::new()), vec![(None, ServerMessage::Timers(vec![]))]);
    assert!(telemetry.flush(now + INTERVAL * 7, &mut engine, &HashMap::new()).is_empty());
}

#[test]
fn node_errors_emit_changes_once_per_interval_and_preserve_counts() {
    let now = Instant::now();
    let mut telemetry = Telemetry::new(now);
    let mut engine = ticknode_core::TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "const", &[]).unwrap();
    let fail = |engine: &mut ticknode_core::TickEngine| {
        engine.host_state.error(id, ticknode_shared::UiMessage::plain("Failed"), "[ERROR] Failed".into());
    };
    fail(&mut engine);
    let messages = telemetry.flush(now, &mut engine, &HashMap::new());
    let [(_, ServerMessage::NodeErrors { set, cleared })] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(set[0].node, id);
    assert_eq!(set[0].count, 1);
    assert!(cleared.is_empty());
    fail(&mut engine);
    fail(&mut engine);
    assert!(telemetry.flush(now + INTERVAL / 2, &mut engine, &HashMap::new()).is_empty());
    let messages = telemetry.flush(now + INTERVAL, &mut engine, &HashMap::new());
    let [(_, ServerMessage::NodeErrors { set, .. })] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(set.len(), 1);
    assert_eq!(set[0].count, 3);
    assert!(telemetry.flush(now + INTERVAL * 2, &mut engine, &HashMap::new()).is_empty());
    engine.set_node_property(id, "value", 42i64.into()).unwrap();
    assert_eq!(telemetry.flush(now + INTERVAL * 3, &mut engine, &HashMap::new()), vec![(None, ServerMessage::NodeErrors { set: vec![], cleared: vec![id] })]);
    assert!(telemetry.flush(now + INTERVAL * 4, &mut engine, &HashMap::new()).is_empty());
}
