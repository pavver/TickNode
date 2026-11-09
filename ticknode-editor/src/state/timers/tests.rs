use super::*;
use ticknode_shared::HistoryState;

fn timer(node: NodeId, remaining_ms: u64, count: u32) -> NodeTimer {
    NodeTimer { node, total_ms: 1000, remaining_ms, count }
}

fn init(timers: Vec<NodeTimer>) -> ServerMessage {
    ServerMessage::Init {
        definitions: vec![], nodes: vec![], exec_connections: vec![], data_connections: vec![],
        traces: vec![], types: vec![], plugins: vec![], translations: vec![], property_options: vec![],
        history: HistoryState::default(), wire_routes: vec![], detached_connections: vec![], timers,
    }
}

#[test]
fn snapshots_replace_timers_and_rebase_countdowns_including_identical_restarts() {
    let (a, b) = (NodeId::generate(), NodeId::generate());
    let mut all = NodeTimers::new();
    let first = timer(a, 750, 1);
    apply(&mut all, &init(vec![first.clone()]), 100.0);
    assert_eq!(all[&a].progress(350.0), (500.0, 0.5));
    apply(&mut all, &ServerMessage::Timers(vec![first.clone(), timer(b, 1000, 2)]), 350.0);
    assert_eq!(all[&a].received_at, 350.0);
    assert_eq!(all[&b].received_at, 350.0);
    let unchanged = all.clone();
    apply(&mut all, &ServerMessage::Timers(vec![first.clone(), timer(b, 1000, 2)]), 350.0);
    assert_eq!(all, unchanged);
    apply(&mut all, &ServerMessage::Timers(vec![first, timer(b, 1000, 2)]), 400.0);
    assert_eq!(all[&a].received_at, 400.0, "a restarted first delay may have identical duration and count");
    apply(&mut all, &ServerMessage::Timers(vec![timer(a, 400, 3)]), 450.0);
    assert_eq!(all.len(), 1);
    assert_eq!(all[&a].received_at, 450.0);
    assert_eq!(all[&a].timer.count, 3);
    // A reconnect refreshes receipt times, including for an identical snapshot.
    apply(&mut all, &init(vec![timer(a, 400, 3)]), 500.0);
    assert_eq!(all[&a].received_at, 500.0);
    apply(&mut all, &init(vec![]), 600.0);
    assert!(all.is_empty());
    apply(&mut all, &ServerMessage::Timers(vec![timer(a, 750, 1)]), 700.0);
    apply(&mut all, &ServerMessage::NodeRemoved(a), 750.0);
    assert!(all.is_empty());
    apply(&mut all, &ServerMessage::Timers(vec![timer(b, 1000, 1)]), 800.0);
    apply(&mut all, &ServerMessage::Timers(vec![]), 900.0);
    assert!(all.is_empty());
}

#[test]
fn progress_handles_completion_and_zero_duration_without_invalid_css_values() {
    let mut entry = ClientTimer { timer: timer(NodeId::generate(), 500, 1), received_at: 100.0 };
    assert_eq!(entry.progress(50.0), (500.0, 0.5));
    assert_eq!(entry.progress(2000.0), (0.0, 0.0));
    entry.timer.total_ms = 0;
    assert_eq!(entry.progress(100.0), (500.0, 0.0));
}
