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
    assert!(telemetry.flush(start).is_empty(), "nothing is sent before the interval passes");

    let messages = telemetry.flush(start + INTERVAL);
    let [ServerMessage::Executed(traces)] = messages.as_slice() else { panic!("{messages:?}") };
    assert_eq!(traces.iter().map(|t| t.node_id).collect::<Vec<_>>(), [a, b]);
    assert_eq!(traces[0].tick, 2);
    assert_eq!(traces[0].inputs, vec![(PinId(1), Some(DataValue::from(2)))]);
    assert_eq!(traces[0].fired, vec![PinId(1), PinId(2)]);

    telemetry.add_traces(&[trace(a, 3, 3, &[])]);
    assert!(telemetry.flush(start + INTERVAL + INTERVAL / 2).is_empty());
    let messages = telemetry.flush(start + INTERVAL * 2);
    let [ServerMessage::Executed(traces)] = messages.as_slice() else { panic!("{messages:?}") };
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
    let messages = telemetry.flush(now);
    assert_eq!(messages.len(), MAX_LOGS + 1);
    assert!(matches!(&messages[0], ServerMessage::Log { message, .. } if message == "line 0"));
    assert!(matches!(&messages[MAX_LOGS - 1], ServerMessage::Log { message, .. } if *message == format!("line {}", MAX_LOGS - 1)));
    assert!(matches!(&messages[MAX_LOGS], ServerMessage::Log { node_id, message } if node_id.0.is_nil() && message.starts_with("[WARN] 6 log lines dropped")));
    assert!(telemetry.flush(now + INTERVAL).is_empty(), "the overflow counter resets");
}

#[test]
fn discard_drops_everything_collected() {
    let now = Instant::now();
    let mut telemetry = Telemetry::new(now);
    telemetry.add_traces(&[trace(NodeId::generate(), 1, 1, &[])]);
    telemetry.add_logs(vec![(NodeId::generate(), "x".into())]);
    telemetry.discard();
    assert!(telemetry.flush(now).is_empty());
}
