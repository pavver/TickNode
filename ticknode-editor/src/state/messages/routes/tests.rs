use super::*;
use ticknode_shared::*;

fn route(exec: bool, c: ConnectionModel) -> WireRoute {
    WireRoute {
        exec,
        connection: c,
        points: vec![Position { x: 16.0, y: 32.0 }],
    }
}

fn apply_twice(all: &mut WireRoutes, event: ServerMessage) {
    assert!(applies(&event));
    apply(all, &event);
    let after = all.clone();
    apply(all, &event);
    assert_eq!(*all, after);
}

#[test]
fn point_updates_and_graph_pruning_are_idempotent_and_preserve_other_wires() {
    let c = ConnectionModel {
        src_node: NodeId::generate(),
        src_pin: PinId(4),
        dst_node: NodeId::generate(),
        dst_pin: PinId(1),
    };
    let other = ConnectionModel {
        dst_node: NodeId::generate(),
        ..c
    };
    let mut all = WireRoutes::new();
    for (exec, c) in [(true, c), (false, c), (false, other)] {
        apply_twice(&mut all, ServerMessage::WireRouted(route(exec, c)));
    }
    apply_twice(&mut all, ServerMessage::DataConnected(c));
    assert_eq!(
        all.len(),
        3,
        "a duplicate connection event must preserve its route"
    );
    let replacement = ConnectionModel {
        src_node: NodeId::generate(),
        ..c
    };
    apply_twice(&mut all, ServerMessage::DataConnected(replacement));
    assert_eq!(all.len(), 2);
    assert!(all.contains_key(&(true, c)));
    assert!(all.contains_key(&(false, other)));
    apply_twice(&mut all, ServerMessage::ExecDisconnected(c));
    assert_eq!(all.len(), 1);
    apply_twice(
        &mut all,
        ServerMessage::DataDisconnected {
            dst_node: other.dst_node,
            dst_pin: other.dst_pin,
        },
    );
    assert!(all.is_empty());
    apply_twice(&mut all, ServerMessage::WireRouted(route(true, c)));
    apply_twice(
        &mut all,
        ServerMessage::WireRouted(WireRoute {
            points: vec![],
            ..route(true, c)
        }),
    );
    assert!(all.is_empty());
    for id in [c.src_node, c.dst_node] {
        apply_twice(&mut all, ServerMessage::WireRouted(route(true, c)));
        apply_twice(&mut all, ServerMessage::NodeRemoved(id));
        assert!(all.is_empty());
    }
}

#[test]
fn init_replaces_all_routes_including_an_empty_snapshot() {
    let c = ConnectionModel {
        src_node: NodeId::generate(),
        src_pin: PinId(4),
        dst_node: NodeId::generate(),
        dst_pin: PinId(0),
    };
    let snapshot = |wire_routes| ServerMessage::Init {
        node_errors: vec![],
        definitions: vec![],
        nodes: vec![],
        exec_connections: vec![c],
        data_connections: vec![],
        traces: vec![],
        types: vec![],
        plugins: vec![],
        translations: vec![],
        property_options: vec![],
        history: HistoryState::default(),
        wire_routes,
        detached_connections: Vec::new(),
        timers: vec![],
        frames: vec![],
        sheets: vec![],
        open_sheet: None,
        variables: vec![],
        server_settings: ServerSettings::default(),
    };
    let mut all = WireRoutes::new();
    apply_twice(&mut all, ServerMessage::WireRouted(route(false, c)));
    apply_twice(&mut all, snapshot(vec![route(true, c)]));
    assert_eq!(all.len(), 1);
    assert!(all.contains_key(&(true, c)));
    apply_twice(&mut all, snapshot(vec![]));
    assert!(all.is_empty());
    assert!(!applies(&ServerMessage::History(HistoryState::default())));
}
