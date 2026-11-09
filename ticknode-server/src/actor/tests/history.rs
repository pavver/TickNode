use super::*;

fn actor(dir: &std::path::Path) -> Actor {
    let (events, _) = broadcast::channel(256);
    Actor {
        engine: TickEngine::new(), positions: HashMap::new(), sizes: HashMap::new(), routes: HashMap::new(),
        storage: Storage::new(dir.join("graph.json")), plugins: Plugins::new(dir),
        events, dirty_since: None, traces: HashMap::new(), history: Default::default(), telemetry: super::telemetry::Telemetry::new(std::time::Instant::now()),
    }
}

fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, &reply).unwrap().0
}

fn add(actor: &mut Actor, x: f32) -> NodeId {
    apply(actor, ClientMessage::AddNode { type_name: "for_loop".into(), position: Position { x, y: 20.0 } })
        .into_iter().find_map(|e| match e { ServerMessage::NodeAdded(n) => Some(n.model.id), _ => None }).unwrap()
}

fn state(actor: &Actor, undo: bool, redo: bool) {
    let ServerMessage::Init { history, .. } = actor.snapshot() else { panic!("missing Init") };
    assert_eq!(history, HistoryState { can_undo: undo, can_redo: redo });
}

#[test]
fn history_add_undo_redo_same_id_and_protocol_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    state(&actor, false, false);
    let id = add(&mut actor, 12.0);
    state(&actor, true, false);
    let events = apply(&mut actor, ClientMessage::Undo);
    assert!(matches!(events.first(), Some(ServerMessage::NodeRemoved(n)) if *n == id));
    assert!(matches!(events.last(), Some(ServerMessage::History(HistoryState { can_undo: false, can_redo: true }))));
    state(&actor, false, true);
    assert!(!actor.engine.has_node(id));
    let events = apply(&mut actor, ClientMessage::Redo);
    assert!(matches!(events.first(), Some(ServerMessage::NodeAdded(n)) if n.model.id == id && n.model.position.x == 12.0));
    state(&actor, true, false);
    for event in events.into_iter().chain([actor.snapshot()]) {
        assert_eq!(postcard::from_bytes::<ServerMessage>(&encode(&event)).unwrap(), event);
    }
}

#[test]
fn history_batch_restores_nodes_positions_and_connections_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 10.0);
    let b = add(&mut actor, 80.0);
    let c = ConnectionModel { src_node: a, src_pin: PinId(4), dst_node: b, dst_pin: PinId(0) };
    apply(&mut actor, ClientMessage::ConnectExec(c));
    let d = ConnectionModel { src_pin: PinId(5), dst_pin: PinId(1), ..c };
    apply(&mut actor, ClientMessage::ConnectData(d));
    let before = actor.graph_snapshot();
    let msg = ClientMessage::Batch(vec![ClientMessage::RemoveNode(a), ClientMessage::RemoveNode(b)]);
    assert_eq!(postcard::from_bytes::<ClientMessage>(&postcard::to_allocvec(&msg).unwrap()).unwrap(), msg);
    apply(&mut actor, msg);
    assert!(actor.engine.node_ids().is_empty());
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert_eq!(events.iter().filter(|e| matches!(e, ServerMessage::NodeAdded(_))).count(), 2);
    assert!(events.iter().any(|e| matches!(e, ServerMessage::ExecConnected(v) if *v == c)));
    assert!(events.iter().any(|e| matches!(e, ServerMessage::DataConnected(v) if *v == d)));
    apply(&mut actor, ClientMessage::Redo);
    assert!(actor.engine.node_ids().is_empty());
}

#[test]
fn history_new_edit_clears_redo_but_failures_and_noops_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = add(&mut actor, 10.0);
    apply(&mut actor, ClientMessage::Undo);
    let (reply, _) = mpsc::unbounded_channel();
    assert!(actor.apply(ClientMessage::RemoveNode(id), &reply).is_err());
    apply(&mut actor, ClientMessage::DisconnectData { dst_node: id, dst_pin: PinId(1) });
    state(&actor, false, true);
    let id = add(&mut actor, 40.0);
    state(&actor, true, false);
    apply(&mut actor, ClientMessage::MoveNode { id, position: actor_position(40.0) });
    apply(&mut actor, ClientMessage::Undo);
    assert!(!actor.engine.has_node(id), "an unchanged move must not add history");
}

fn actor_position(x: f32) -> Position { Position { x, y: 20.0 } }

#[test]
fn history_batch_keeps_successes_skips_forbidden_and_reports_first_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let (reply, mut errors) = mpsc::unbounded_channel();
    let (events, _) = actor.apply(ClientMessage::Batch(vec![
        ClientMessage::Undo, ClientMessage::Redo, ClientMessage::Batch(vec![ClientMessage::RemoveNode(a)]),
        ClientMessage::TriggerNode { id: a, pin: PinId(0) },
        ClientMessage::SetPluginEnabled { name: "missing".into(), enabled: false },
        ClientMessage::ConfigurePlugin { name: "missing".into(), values: vec![] },
        ClientMessage::RemoveNode(a), ClientMessage::RemoveNode(a), ClientMessage::RemoveNode(b),
    ]), &reply).unwrap();
    assert_eq!(events.iter().filter(|e| matches!(e, ServerMessage::NodeRemoved(_))).count(), 2);
    assert!(matches!(errors.try_recv(), Ok(ServerMessage::Error(e)) if e.key == "error-batch-command"));
    assert!(errors.try_recv().is_err());
    assert!(actor.engine.execution_queue.is_empty());
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.engine.has_node(a) && actor.engine.has_node(b));
}

#[test]
fn history_plugin_toggle_clears_both_stacks() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
    std::fs::copy(wasm, dir.path().join("plugins/test.wasm")).unwrap();
    actor.plugins.load_all(&mut actor.engine);
    let name = actor.plugins.infos(&actor.engine)[0].name.clone();
    let events = apply(&mut actor, ClientMessage::ConfigurePlugin {
        name: name.clone(), values: vec![("greeting".into(), SettingUpdate::Set(DataValue::from("Hello actor")))],
    });
    assert!(matches!(&events[0], ServerMessage::PluginUpdated(info) if info.values.contains(&(
        "greeting".into(), SettingValueView::Value(DataValue::from("Hello actor")),
    ))));
    add(&mut actor, 1.0);
    add(&mut actor, 2.0);
    apply(&mut actor, ClientMessage::Undo);
    state(&actor, true, true);
    let events = apply(&mut actor, ClientMessage::SetPluginEnabled { name, enabled: false });
    state(&actor, false, false);
    assert!(matches!(events.last(), Some(ServerMessage::History(HistoryState { can_undo: false, can_redo: false }))));
}

#[test]
fn connection_commands_only_broadcast_actual_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let exec = ConnectionModel { src_node: a, src_pin: PinId(4), dst_node: b, dst_pin: PinId(0) };
    let data = ConnectionModel { src_pin: PinId(5), dst_pin: PinId(1), ..exec };
    assert!(matches!(apply(&mut actor, ClientMessage::ConnectExec(exec))[0], ServerMessage::ExecConnected(c) if c == exec));
    assert!(apply(&mut actor, ClientMessage::ConnectExec(exec)).is_empty());
    assert!(matches!(apply(&mut actor, ClientMessage::ConnectData(data))[0], ServerMessage::DataConnected(c) if c == data));
    assert!(apply(&mut actor, ClientMessage::ConnectData(data)).is_empty());
    assert!(matches!(apply(&mut actor, ClientMessage::DisconnectExec(exec))[0], ServerMessage::ExecDisconnected(c) if c == exec));
    assert!(apply(&mut actor, ClientMessage::DisconnectExec(exec)).is_empty());
    let disconnect = ClientMessage::DisconnectData { dst_node: b, dst_pin: PinId(1) };
    assert!(matches!(apply(&mut actor, disconnect.clone())[0], ServerMessage::DataDisconnected { dst_node, dst_pin } if dst_node == b && dst_pin == PinId(1)));
    assert!(apply(&mut actor, disconnect).is_empty());
    assert!(actor.engine.exec_connections().is_empty());
    assert!(actor.engine.data_connections().is_empty());
}

#[test]
fn paste_returns_created_ids_positions_warnings_and_limit_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let mut snippet = actor.graph_snapshot();
    snippet.data_connections.push(ConnectionModel { src_node: a, src_pin: PinId(99), dst_node: b, dst_pin: PinId(1) });
    // Invalid pins detach; a cycle remains a paste warning.
    snippet.data_connections.push(ConnectionModel { src_node: a, src_pin: PinId(5), dst_node: a, dst_pin: PinId(1) });
    let expected_positions = snippet.nodes.iter().map(|node| node.position).collect::<Vec<_>>();
    let (reply, mut replies) = mpsc::unbounded_channel();
    let (events, created) = actor.apply(ClientMessage::Paste(snippet.clone()), &reply).unwrap();
    let ids = created.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| *id != a && *id != b));
    let positions = events.iter().filter_map(|event| match event {
        ServerMessage::NodeAdded(node) => Some(node.model.position), _ => None,
    }).collect::<Vec<_>>();
    assert_eq!(positions, expected_positions);
    assert!(events.iter().any(|event| matches!(event, ServerMessage::Detached(_))));
    let Ok(ServerMessage::Error(warning)) = replies.try_recv() else { panic!("missing paste warning") };
    assert_eq!(warning.key, "error-paste-incomplete");
    assert!(warning.fallback.contains("Data connection"));
    let node = snippet.nodes[0].clone();
    snippet.nodes = vec![node; MAX_PASTE_NODES + 1];
    let error = actor.apply(ClientMessage::Paste(snippet), &reply).unwrap_err();
    assert_eq!(error.key, "error-paste-too-many");
    assert_eq!(error.args, vec![("count".into(), "1001".into()), ("max".into(), "1000".into())]);
    assert_eq!(actor.engine.node_ids().len(), 4);
}

#[tokio::test]
async fn actor_ticks_broadcast_logs_and_keep_traces_in_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "console", &[]).unwrap();
    engine.set_input_value(id, PinId(1), Some(DataValue::from("tick output"))).unwrap();
    let handle = spawn(engine, Default::default(), Storage::new(dir.path().join("graph.json")),
        Plugins::new(dir.path()), Duration::from_millis(5)).unwrap();
    let mut events = handle.subscribe();
    let (reply, _) = mpsc::unbounded_channel();
    handle.send(ClientMessage::TriggerNode { id, pin: PinId(0) }, reply);
    let mut executed = false;
    let mut logged = false;
    while !executed || !logged {
        let bytes = tokio::time::timeout(Duration::from_secs(5), events.recv()).await.unwrap().unwrap();
        match postcard::from_bytes::<ServerMessage>(&bytes).unwrap() {
            ServerMessage::Executed(traces) => {
                assert!(traces.iter().any(|trace| trace.node_id == id && trace.inputs.contains(&(PinId(1), Some(DataValue::from("tick output"))))));
                executed = true;
            }
            ServerMessage::Log { node_id, message } => {
                assert_eq!((node_id, message.as_str()), (id, "tick output"));
                logged = true;
            }
            _ => {},
        }
    }
    let Some(ServerMessage::Init { traces, .. }) = handle.snapshot().await else { panic!("missing snapshot") };
    assert!(traces.iter().any(|(trace, _)| trace.node_id == id));
    handle.shutdown().await;
}

#[test]
fn history_bounds_entries_and_empty_undo_redo_do_not_emit_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    assert!(apply(&mut actor, ClientMessage::Undo).is_empty());
    assert!(apply(&mut actor, ClientMessage::Redo).is_empty());
    let first = add(&mut actor, 0.0);
    for x in 1..=100 { add(&mut actor, x as f32); }
    for _ in 0..100 { apply(&mut actor, ClientMessage::Undo); }
    state(&actor, false, true);
    assert_eq!(actor.engine.node_ids(), vec![first]);
    assert!(apply(&mut actor, ClientMessage::Undo).is_empty());
    for _ in 0..100 { apply(&mut actor, ClientMessage::Redo); }
    state(&actor, true, false);
    assert_eq!(actor.engine.node_ids().len(), 101);
}

#[test]
fn history_batch_collects_created_ids_and_keeps_first_command_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let (reply, mut errors) = mpsc::unbounded_channel();
    let missing = NodeId::generate();
    let (_, ids) = actor.apply(ClientMessage::Batch(vec![
        ClientMessage::RemoveNode(missing), ClientMessage::Undo,
        ClientMessage::AddNode { type_name: "const".into(), position: Position::default() },
        ClientMessage::AddNode { type_name: "for_loop".into(), position: Position::default() },
    ]), &reply).unwrap();
    let ids = ids.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| actor.engine.has_node(*id)));
    assert!(matches!(errors.try_recv(), Ok(ServerMessage::Error(e)) if e == EngineError::NodeNotFound(missing).ui_message()));
    assert!(errors.try_recv().is_err());
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.engine.node_ids().is_empty());
}

#[test]
fn history_moves_kept_nodes_and_recreates_changed_nodes_in_event_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let exec = ConnectionModel { src_node: a, src_pin: PinId(4), dst_node: b, dst_pin: PinId(0) };
    let data = ConnectionModel { src_pin: PinId(5), dst_pin: PinId(1), ..exec };
    apply(&mut actor, ClientMessage::ConnectExec(exec));
    apply(&mut actor, ClientMessage::ConnectData(data));
    let task = actor.engine.host_state.start_delay(a, 100);
    let trace = NodeTrace { node_id: b, tick: 0, inputs: vec![], outputs: vec![], fired: vec![] };
    actor.traces.insert(b, (trace, Instant::now()));
    apply(&mut actor, ClientMessage::Batch(vec![
        ClientMessage::SetInputValue { id: b, pin: PinId(1), value: Some(DataValue::from(8.0)) },
        ClientMessage::MoveNode { id: a, position: actor_position(50.0) },
    ]));
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.positions[&a], actor_position(1.0));
    assert!(actor.engine.host_state.active_tasks.contains_key(&task));
    assert!(!actor.traces.contains_key(&b));
    assert!(matches!(events.as_slice(), [
        ServerMessage::NodeUpdated(n), ServerMessage::NodeMoved { id: moved, .. }, ServerMessage::History(_),
    ] if n.model.id == b && *moved == a));
    assert_eq!(actor.engine.exec_connections(), &[exec]);
    assert_eq!(actor.engine.data_connections(), &[data]);
}

#[tokio::test]
async fn history_is_broadcast_to_all_editors_and_in_new_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let handle = spawn(TickEngine::new(), Default::default(), Storage::new(dir.path().join("graph.json")),
        Plugins::new(dir.path()), Duration::from_secs(10)).unwrap();
    let mut first = handle.subscribe();
    let mut second = handle.subscribe();
    let (reply, mut replies) = mpsc::unbounded_channel();
    handle.send(ClientMessage::AddNode { type_name: "const".into(), position: Position::default() }, reply);
    for receiver in [&mut first, &mut second] {
        let node = tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await.unwrap().unwrap();
        assert!(matches!(postcard::from_bytes::<ServerMessage>(&node).unwrap(), ServerMessage::NodeAdded(_)));
        let history = tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(postcard::from_bytes::<ServerMessage>(&history).unwrap(), ServerMessage::History(HistoryState { can_undo: true, can_redo: false }));
    }
    assert!(matches!(replies.recv().await, Some(ServerMessage::Created(_))));
    assert!(matches!(handle.snapshot().await, Some(ServerMessage::Init { history: HistoryState { can_undo: true, can_redo: false }, .. })));
    handle.shutdown().await;
}

#[tokio::test]
async fn add_node_with_creates_node_with_properties_replies_created_and_supports_undo() {
    let dir = tempfile::tempdir().unwrap();
    let handle = spawn(TickEngine::new(), Default::default(), Storage::new(dir.path().join("graph.json")),
        Plugins::new(dir.path()), Duration::from_secs(10)).unwrap();
    let (reply, mut replies) = mpsc::unbounded_channel();
    let pos = Position { x: 42.0, y: 84.0 };
    handle.send(ClientMessage::AddNodeWith {
        type_name: "const".into(),
        position: pos,
        properties: vec![("value".into(), DataValue::from(123.0))],
    }, reply);

    let created_id = match replies.recv().await {
        Some(ServerMessage::Created(ids)) => {
            assert_eq!(ids.len(), 1);
            ids[0]
        }
        other => panic!("Expected Created, got {other:?}"),
    };

    let snapshot = handle.snapshot().await.unwrap();
    let ServerMessage::Init { nodes, .. } = snapshot else { panic!("Expected Init") };
    let node = nodes.iter().find(|n| n.model.id == created_id).expect("node exists");
    assert_eq!(node.model.position, pos);
    assert_eq!(node.model.properties, vec![("value".into(), DataValue::from(123.0))]);

    // Undo removes it
    let (undo_reply, _) = mpsc::unbounded_channel();
    handle.send(ClientMessage::Undo, undo_reply);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let snapshot = handle.snapshot().await.unwrap();
    let ServerMessage::Init { nodes, .. } = snapshot else { panic!("Expected Init") };
    assert!(!nodes.iter().any(|n| n.model.id == created_id));

    // Rejects in:* and split:*
    let (err_reply, mut err_replies) = mpsc::unbounded_channel();
    handle.send(ClientMessage::AddNodeWith {
        type_name: "const".into(),
        position: Position::default(),
        properties: vec![("in:0".into(), DataValue::Bool(true))],
    }, err_reply);
    match err_replies.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-invalid-property");
            assert_eq!(err.args, vec![("property".into(), "in:0".into())]);
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    let (err_reply2, mut err_replies2) = mpsc::unbounded_channel();
    handle.send(ClientMessage::AddNodeWith {
        type_name: "const".into(),
        position: Position::default(),
        properties: vec![("split:0".into(), DataValue::Bool(true))],
    }, err_reply2);
    match err_replies2.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-invalid-property");
            assert_eq!(err.args, vec![("property".into(), "split:0".into())]);
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    // Unknown node type returns error
    let (err_reply3, mut err_replies3) = mpsc::unbounded_channel();
    handle.send(ClientMessage::AddNodeWith {
        type_name: "unknown_type".into(),
        position: Position::default(),
        properties: vec![],
    }, err_reply3);
    match err_replies3.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-unknown-node-type");
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    handle.shutdown().await;
}
