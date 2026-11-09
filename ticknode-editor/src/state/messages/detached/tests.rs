use super::*;
use ticknode_shared::*;

fn wire() -> DetachedConnection {
    DetachedConnection { exec: false, end: DetachedEnd::Target, row: 2, connection: ConnectionModel {
        src_node: NodeId::generate(), src_pin: PinId(1), dst_node: NodeId::generate(), dst_pin: PinId(2),
    } }
}

fn twice(all: &mut Vec<DetachedConnection>, event: ServerMessage) {
    assert!(applies(&event));
    apply(all, &event);
    let after = all.clone();
    apply(all, &event);
    assert_eq!(*all, after);
}

#[test]
fn duplicate_events_keep_distinct_wires_and_removal_prunes_either_node() {
    let d = wire();
    let exec = DetachedConnection { exec: true, ..d };
    let other = wire();
    let mut all = vec![other];
    twice(&mut all, ServerMessage::Detached(d));
    twice(&mut all, ServerMessage::Detached(exec));
    assert_eq!(all, [other, d, exec]);
    twice(&mut all, ServerMessage::DetachedRemoved(d));
    assert_eq!(all, [other, exec]);
    // Confirming a contract changes a node, never the detached wire collection.
    assert!(!applies(&ServerMessage::History(HistoryState::default())));
    for id in [d.connection.src_node, d.connection.dst_node] {
        twice(&mut all, ServerMessage::Detached(d));
        twice(&mut all, ServerMessage::Detached(exec));
        twice(&mut all, ServerMessage::NodeRemoved(id));
        assert_eq!(all, [other]);
    }
}

#[test]
fn reconnect_snapshot_replaces_detached_wires_including_empty_snapshots() {
    let d = wire();
    let init = |detached_connections| ServerMessage::Init {
        definitions: vec![], nodes: vec![], exec_connections: vec![], data_connections: vec![],
        traces: vec![], types: vec![], plugins: vec![], translations: vec![], property_options: vec![],
        history: HistoryState::default(), wire_routes: vec![], detached_connections, timers: vec![],
    };
    let mut all = vec![wire()];
    twice(&mut all, init(vec![d, d]));
    assert_eq!(all, [d]);
    twice(&mut all, init(vec![]));
    assert!(all.is_empty());
}
