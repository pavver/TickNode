use super::*;

fn actor(dir: &std::path::Path) -> Actor {
    let (events, _) = broadcast::channel(64);
    Actor {
        engine: TickEngine::new(), positions: HashMap::new(), routes: HashMap::new(),
        storage: Storage::new(dir.join("graph.json")), plugins: Plugins::new(dir), events,
        dirty_since: None, traces: HashMap::new(), history: Default::default(),
        telemetry: super::telemetry::Telemetry::new(Instant::now()),
    }
}

fn seed(actor: &mut Actor, kind: &str) -> NodeId {
    let id = NodeId::generate();
    actor.engine.create_node(id, kind, &[]).unwrap();
    id
}

fn command(node: NodeId, from_pin: u32, kind: &str, new_pin: u32) -> ClientMessage {
    ClientMessage::AddNodeConnected {
        type_name: kind.into(), position: Position { x: 125.0, y: 238.0 },
        from: ConnectionEnd { node, pin: PinId(from_pin) }, pin: PinId(new_pin),
    }
}

fn apply(actor: &mut Actor, command: ClientMessage) -> (Vec<ServerMessage>, NodeId) {
    let bytes = postcard::to_allocvec(&command).unwrap();
    assert_eq!(postcard::from_bytes::<ClientMessage>(&bytes).unwrap(), command);
    let (reply, _) = mpsc::unbounded_channel();
    let (events, created) = actor.apply(command, &reply).unwrap();
    let created = created.unwrap();
    assert_eq!(created.len(), 1);
    let id = created[0];
    assert!(matches!(events.first(), Some(ServerMessage::NodeAdded(n))
        if n.model.id == id && n.model.position == Position { x: 125.0, y: 238.0 }));
    (events, id)
}

#[test]
fn output_data_creation_and_connection_are_one_undoable_edit() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let source = seed(&mut actor, "for_loop");
    let before = actor.graph_snapshot();
    let (events, id) = apply(&mut actor, command(source, 5, "for_loop", 1));
    let c = ConnectionModel { src_node: source, src_pin: PinId(5), dst_node: id, dst_pin: PinId(1) };
    assert!(events.contains(&ServerMessage::DataConnected(c)));
    assert_eq!(actor.engine.data_connections(), &[c]);
    let (reply, _) = mpsc::unbounded_channel();
    let (events, _) = actor.apply(ClientMessage::Undo, &reply).unwrap();
    assert!(events.contains(&ServerMessage::NodeRemoved(id)));
    assert_eq!(actor.graph_snapshot(), before);
    assert!(!actor.history.state().can_undo);
    actor.apply(ClientMessage::Redo, &reply).unwrap();
    assert_eq!(actor.engine.data_connections(), &[c]);
}

#[test]
fn input_drag_reverses_direction_and_reports_replaced_source() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sink = seed(&mut actor, "for_loop");
    let source = seed(&mut actor, "for_loop");
    actor.engine.connect_data(source, PinId(5), sink, PinId(1)).unwrap();
    let before = actor.graph_snapshot();
    let (events, id) = apply(&mut actor, command(sink, 1, "for_loop", 5));
    let c = ConnectionModel { src_node: id, src_pin: PinId(5), dst_node: sink, dst_pin: PinId(1) };
    assert!(matches!(events.as_slice(), [ServerMessage::NodeAdded(_),
        ServerMessage::DataDisconnected { dst_node, dst_pin: PinId(1) },
        ServerMessage::DataConnected(connected), ServerMessage::History(_)] if *dst_node == sink && *connected == c));
    assert_eq!(actor.engine.data_connections(), &[c]);
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(ClientMessage::Undo, &reply).unwrap();
    assert_eq!(actor.graph_snapshot(), before);
}

#[test]
fn exec_drag_works_in_both_directions_and_in_batch() {
    for reverse in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut actor = actor(dir.path());
        let existing = seed(&mut actor, "for_loop");
        let cmd = if reverse { command(existing, 0, "for_loop", 4) } else { command(existing, 4, "for_loop", 0) };
        let (events, id) = apply(&mut actor, ClientMessage::Batch(vec![cmd]));
        let c = if reverse {
            ConnectionModel { src_node: id, src_pin: PinId(4), dst_node: existing, dst_pin: PinId(0) }
        } else {
            ConnectionModel { src_node: existing, src_pin: PinId(4), dst_node: id, dst_pin: PinId(0) }
        };
        assert!(events.contains(&ServerMessage::ExecConnected(c)));
        assert_eq!(actor.engine.exec_connections(), &[c]);
    }
}

#[test]
fn rejection_rolls_back_node_positions_events_and_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    actor.engine.register_types(&[TypeDefinition::new_struct("test.point", "Point",
        vec![FieldDefinition::new("x", "X", DataType::Number)])], &[]).unwrap();
    let existing = seed(&mut actor, "for_loop");
    let old_source = seed(&mut actor, "for_loop");
    actor.engine.connect_data(old_source, PinId(5), existing, PinId(1)).unwrap();
    let before = actor.graph_snapshot();
    let (reply, mut replies) = mpsc::unbounded_channel();
    for cmd in [command(existing, 5, "for_loop", 0), command(existing, 5, "for_loop", 999),
        command(existing, 999, "for_loop", 1), command(NodeId::generate(), 0, "for_loop", 1),
        command(existing, 5, "missing", 0), command(existing, 1, "type/make/test.point", 0)] {
        assert!(actor.apply(cmd, &reply).is_err());
        assert_eq!(actor.graph_snapshot(), before);
        assert!(actor.positions.is_empty());
        assert!(!actor.history.state().can_undo);
        assert!(actor.dirty_since.is_none());
        assert!(replies.try_recv().is_err());
    }
}

#[test]
fn initiator_receives_created_after_broadcast_and_failure_only_returns_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let source = seed(&mut actor, "for_loop");
    let mut broadcast = actor.events.subscribe();
    let (reply, mut replies) = mpsc::unbounded_channel();
    actor.handle_command(Command::Client { msg: command(source, 5, "for_loop", 1), reply: reply.clone() });
    let ServerMessage::NodeAdded(node) = postcard::from_bytes(&broadcast.try_recv().unwrap()).unwrap() else { panic!("missing NodeAdded") };
    assert!(matches!(postcard::from_bytes::<ServerMessage>(&broadcast.try_recv().unwrap()).unwrap(), ServerMessage::DataConnected(_)));
    assert!(matches!(postcard::from_bytes::<ServerMessage>(&broadcast.try_recv().unwrap()).unwrap(), ServerMessage::History(_)));
    assert_eq!(replies.try_recv().unwrap(), ServerMessage::Created(vec![node.model.id]));
    actor.handle_command(Command::Client { msg: command(source, 5, "for_loop", 0), reply });
    assert!(matches!(replies.try_recv().unwrap(), ServerMessage::Error(_)));
    assert!(broadcast.try_recv().is_err());
}

#[test]
fn split_field_origin_uses_its_effective_data_type() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    actor.engine.register_types(&[TypeDefinition::new_struct("test.point", "Point",
        vec![FieldDefinition::new("x", "X", DataType::Number)])], &[]).unwrap();
    let source = seed(&mut actor, "type/make/test.point");
    actor.engine.set_pin_split(source, PinId(0), true).unwrap();
    let field = split_field_pin(PinId(0), 0).unwrap();
    let (events, id) = apply(&mut actor, command(source, field.0, "for_loop", 1));
    let c = ConnectionModel { src_node: source, src_pin: field, dst_node: id, dst_pin: PinId(1) };
    assert!(events.contains(&ServerMessage::DataConnected(c)));
    assert_eq!(actor.engine.data_connections(), &[c]);
}
