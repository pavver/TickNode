use super::*;

fn actor(dir: &std::path::Path) -> (Actor, ConnectionModel, ConnectionModel) {
    let (events, _) = broadcast::channel(256);
    let mut actor = Actor {
        engine: TickEngine::new(), positions: HashMap::new(), sizes: HashMap::new(), routes: HashMap::new(),
        storage: Storage::new(dir.join("graph.json")), plugins: Plugins::new(dir),
        events, dirty_since: None, traces: HashMap::new(), history: Default::default(),
        telemetry: super::telemetry::Telemetry::new(Instant::now()),
    };
    let a = NodeId::generate();
    let b = NodeId::generate();
    for id in [a, b] { actor.engine.create_node(id, "for_loop", &[]).unwrap(); }
    let exec = ConnectionModel { src_node: a, src_pin: PinId(4), dst_node: b, dst_pin: PinId(0) };
    let data = ConnectionModel { src_pin: PinId(5), dst_pin: PinId(1), ..exec };
    actor.engine.connect_exec(a, exec.src_pin, b, exec.dst_pin).unwrap();
    actor.engine.connect_data(a, data.src_pin, b, data.dst_pin).unwrap();
    (actor, exec, data)
}

fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, &reply).unwrap().0
}

fn route(exec: bool, connection: ConnectionModel, x: f32) -> WireRoute {
    WireRoute { exec, connection, points: vec![Position { x, y: 80.0 }] }
}

#[test]
fn set_clear_noop_history_and_protocol_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    for exec in [false, true] {
        let (mut actor, e, d) = actor(dir.path());
        let route = route(exec, if exec { e } else { d }, 40.0);
        let command = ClientMessage::SetWireRoute(route.clone());
        assert_eq!(postcard::from_bytes::<ClientMessage>(&postcard::to_allocvec(&command).unwrap()).unwrap(), command);
        let events = apply(&mut actor, command.clone());
        assert_eq!(events[0], ServerMessage::WireRouted(route.clone()));
        assert!(actor.dirty_since.is_some());
        assert!(actor.engine.export_graph().wire_routes.is_empty());
        assert_eq!(actor.graph_snapshot().wire_routes, [route.clone()]);
        let ServerMessage::Init { wire_routes, .. } = actor.snapshot() else { panic!() };
        assert_eq!(wire_routes, [route.clone()]);
        for event in [events[0].clone(), actor.snapshot()] {
            assert_eq!(postcard::from_bytes::<ServerMessage>(&encode(&event)).unwrap(), event);
        }
        actor.dirty_since = None;
        assert!(apply(&mut actor, command).is_empty());
        assert!(actor.dirty_since.is_none());
        let cleared = WireRoute { points: vec![], ..route.clone() };
        let events = apply(&mut actor, ClientMessage::SetWireRoute(cleared.clone()));
        assert_eq!(events[0], ServerMessage::WireRouted(cleared.clone()));
        assert!(actor.routes.is_empty());
        assert!(apply(&mut actor, ClientMessage::SetWireRoute(cleared)).is_empty());
        apply(&mut actor, ClientMessage::Undo);
        assert_eq!(actor.graph_snapshot().wire_routes, [route]);
        apply(&mut actor, ClientMessage::Undo);
        assert!(actor.routes.is_empty(), "identical edits must not create history entries");
    }
}

#[test]
fn rejects_unknown_connections_excess_points_and_nonfinite_coordinates() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, e, d) = actor(dir.path());
    let invalid = [
        WireRoute { connection: ConnectionModel { src_pin: PinId(999), ..e }, ..route(true, e, 0.0) },
        route(false, e, 0.0),
        WireRoute { points: vec![Position::default(); MAX_WIRE_POINTS + 1], ..route(true, e, 0.0) },
        route(true, e, f32::NAN), route(false, d, f32::INFINITY),
        WireRoute { points: vec![Position { x: 0.0, y: f32::NEG_INFINITY }], ..route(false, d, 0.0) },
    ];
    let (reply, _) = mpsc::unbounded_channel();
    for route in invalid {
        let error = actor.apply(ClientMessage::SetWireRoute(route), &reply).unwrap_err();
        assert_eq!(error.key, "error-wire-route");
        assert!(actor.routes.is_empty());
        assert_eq!(actor.history.state(), HistoryState::default());
        assert!(actor.dirty_since.is_none());
    }
    apply(&mut actor, ClientMessage::SetWireRoute(WireRoute { points: vec![Position::default(); MAX_WIRE_POINTS], ..route(false, d, 0.0) }));
}

#[test]
fn pruning_disconnect_remove_and_replaced_data_source_is_undoable() {
    let dir = tempfile::tempdir().unwrap();
    for mode in 0..4 {
        let (mut actor, e, d) = actor(dir.path());
        apply(&mut actor, ClientMessage::SetWireRoute(route(true, e, 40.0)));
        apply(&mut actor, ClientMessage::SetWireRoute(route(false, d, 44.0)));
        let old = actor.graph_snapshot();
        let command = match mode {
            0 => ClientMessage::DisconnectExec(e),
            1 => ClientMessage::DisconnectData { dst_node: d.dst_node, dst_pin: d.dst_pin },
            2 => ClientMessage::RemoveNode(e.src_node),
            _ => {
                let other = NodeId::generate();
                actor.engine.create_node(other, "for_loop", &[]).unwrap();
                ClientMessage::ConnectData(ConnectionModel { src_node: other, ..d })
            }
        };
        apply(&mut actor, command);
        assert_eq!(actor.routes.contains_key(&(true, e)), mode == 1 || mode == 3);
        assert_eq!(actor.routes.contains_key(&(false, d)), mode == 0);
        let events = apply(&mut actor, ClientMessage::Undo);
        assert_eq!(actor.graph_snapshot().wire_routes, old.wire_routes);
        assert!(events.iter().any(|e| matches!(e, ServerMessage::WireRouted(r) if !r.points.is_empty())));
        apply(&mut actor, ClientMessage::Redo);
        assert_eq!(actor.routes.contains_key(&(false, d)), mode == 0);
    }
}

#[test]
fn point_move_and_group_moves_restore_together_in_one_batch() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, e, _) = actor(dir.path());
    let original = route(true, e, 40.0);
    apply(&mut actor, ClientMessage::SetWireRoute(original.clone()));
    let moved = route(true, e, 80.0);
    apply(&mut actor, ClientMessage::Batch(vec![
        ClientMessage::MoveNode { id: e.src_node, position: Position { x: 40.0, y: 0.0 } },
        ClientMessage::MoveNode { id: e.dst_node, position: Position { x: 40.0, y: 0.0 } },
        ClientMessage::SetWireRoute(moved.clone()),
    ]));
    let after = actor.graph_snapshot();
    let undo = apply(&mut actor, ClientMessage::Undo);
    assert!(undo.contains(&ServerMessage::WireRouted(original.clone())));
    assert_eq!(actor.positions[&e.src_node], Position::default());
    assert_eq!(actor.positions[&e.dst_node], Position::default());
    let redo = apply(&mut actor, ClientMessage::Redo);
    assert!(redo.contains(&ServerMessage::WireRouted(moved)));
    assert_eq!(actor.graph_snapshot(), after);
    apply(&mut actor, ClientMessage::Undo);
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.routes.is_empty());
}

#[test]
fn paste_remaps_routes_and_emits_them_after_connections() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, e, d) = actor(dir.path());
    let mut snippet = actor.engine.export_graph();
    snippet.wire_routes = vec![route(true, e, 40.0), route(false, d, 44.0), route(false, e, 48.0)];
    let events = apply(&mut actor, ClientMessage::Paste(snippet));
    let routes: Vec<_> = events.iter().filter_map(|e| if let ServerMessage::WireRouted(r) = e { Some(r.clone()) } else { None }).collect();
    assert_eq!(routes.len(), 2);
    for r in &routes {
        assert_ne!(r.connection.src_node, e.src_node);
        assert_ne!(r.connection.dst_node, e.dst_node);
        assert_eq!(r.points, route(r.exec, r.connection, if r.exec { 40.0 } else { 44.0 }).points);
    }
    let last_connection = events.iter().rposition(|e| matches!(e, ServerMessage::ExecConnected(_) | ServerMessage::DataConnected(_))).unwrap();
    let first_route = events.iter().position(|e| matches!(e, ServerMessage::WireRouted(_))).unwrap();
    assert!(last_connection < first_route);
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.routes.is_empty());
    apply(&mut actor, ClientMessage::Redo);
    assert_eq!(actor.routes.len(), 2);
}

#[test]
fn disconnect_then_reconnect_in_a_batch_does_not_keep_an_unannounced_route() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, e, _) = actor(dir.path());
    apply(&mut actor, ClientMessage::SetWireRoute(route(true, e, 40.0)));
    apply(&mut actor, ClientMessage::Batch(vec![ClientMessage::DisconnectExec(e), ClientMessage::ConnectExec(e)]));
    assert!(actor.routes.is_empty());
    apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.routes.len(), 1);
}
