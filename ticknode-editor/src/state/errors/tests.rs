use super::*;
use ticknode_shared::{HistoryState, UiMessage};

fn error(node: NodeId, count: u32) -> NodeError {
    NodeError { node, message: UiMessage::plain("Failed"), age_ms: 200, count }
}

#[test]
fn changes_are_idempotent_preserve_age_and_clear_removed_nodes() {
    let id = NodeId::generate();
    let mut all = NodeErrors::new();
    let event = ServerMessage::NodeErrors { set: vec![error(id, 1)], cleared: vec![] };
    apply(&mut all, &event, 1000.0);
    apply(&mut all, &event, 1500.0);
    assert_eq!(all[&id].age_ms(2000.0), 1200.0);
    apply(&mut all, &ServerMessage::NodeErrors { set: vec![error(id, 2)], cleared: vec![] }, 2000.0);
    assert_eq!(all[&id].age_ms(2100.0), 300.0);
    let clear = ServerMessage::NodeErrors { set: vec![], cleared: vec![id] };
    apply(&mut all, &clear, 2200.0);
    apply(&mut all, &clear, 2200.0);
    assert!(all.is_empty());
    apply(&mut all, &event, 2300.0);
    apply(&mut all, &ServerMessage::NodeRemoved(id), 2400.0);
    apply(&mut all, &ServerMessage::NodeRemoved(id), 2400.0);
    assert!(all.is_empty());
}

#[test]
fn reconnect_snapshots_replace_all_errors() {
    let (a, b) = (NodeId::generate(), NodeId::generate());
    let mut all = NodeErrors::new();
    let init = |node_errors| ServerMessage::Init {
        definitions: vec![], nodes: vec![], exec_connections: vec![], data_connections: vec![], traces: vec![],
        types: vec![], plugins: vec![], translations: vec![], property_options: vec![], history: HistoryState::default(),
        wire_routes: vec![], detached_connections: vec![], timers: vec![], node_errors,
    };
    apply(&mut all, &init(vec![error(a, 2)]), 100.0);
    apply(&mut all, &init(vec![error(b, 3)]), 300.0);
    assert_eq!(all.len(), 1);
    assert_eq!(all[&b].error.count, 3);
    assert_eq!(all[&b].age_ms(400.0), 300.0);
    apply(&mut all, &init(vec![]), 500.0);
    assert!(all.is_empty());
}
