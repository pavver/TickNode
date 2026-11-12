use super::*;

#[test]
fn history_batch_restores_nodes_positions_and_connections_once() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 10.0);
    let b = add(&mut actor, 80.0);
    let c = ConnectionModel {
        src_node: a,
        src_pin: PinId(4),
        dst_node: b,
        dst_pin: PinId(0),
    };
    apply(&mut actor, ClientMessage::ConnectExec(c));
    let d = ConnectionModel {
        src_pin: PinId(5),
        dst_pin: PinId(1),
        ..c
    };
    apply(&mut actor, ClientMessage::ConnectData(d));
    let before = actor.graph_snapshot();
    let msg = ClientMessage::Batch(vec![
        ClientMessage::RemoveNode(a),
        ClientMessage::RemoveNode(b),
    ]);
    assert_eq!(
        postcard::from_bytes::<ClientMessage>(&postcard::to_allocvec(&msg).unwrap()).unwrap(),
        msg
    );
    apply(&mut actor, msg);
    assert!(actor.engine.node_ids().is_empty());
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ServerMessage::NodeAdded(_)))
            .count(),
        2
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::ExecConnected(v) if *v == c))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::DataConnected(v) if *v == d))
    );
    apply(&mut actor, ClientMessage::Redo);
    assert!(actor.engine.node_ids().is_empty());
}

#[test]
fn history_batch_keeps_successes_skips_forbidden_and_reports_first_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let (reply, mut errors) = mpsc::unbounded_channel();
    let (events, _) = actor
        .apply(
            ClientMessage::Batch(vec![
                ClientMessage::Undo,
                ClientMessage::Redo,
                ClientMessage::Batch(vec![ClientMessage::RemoveNode(a)]),
                ClientMessage::TriggerNode {
                    id: a,
                    pin: PinId(0),
                },
                ClientMessage::SetPluginEnabled {
                    name: "missing".into(),
                    enabled: false,
                },
                ClientMessage::ConfigurePlugin {
                    name: "missing".into(),
                    values: vec![],
                },
                ClientMessage::RemoveNode(a),
                ClientMessage::RemoveNode(a),
                ClientMessage::RemoveNode(b),
            ]),
            actor.sheets.first().map(|s| s.id),
            &reply,
        )
        .unwrap();
    let events: Vec<ServerMessage> = events.into_iter().map(|(_, m)| m).collect();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ServerMessage::NodeRemoved(_)))
            .count(),
        2
    );
    assert!(
        matches!(errors.try_recv(), Ok(ServerMessage::Error(e)) if e.key == "error-batch-command")
    );
    assert!(errors.try_recv().is_err());
    assert!(actor.engine.execution_queue.is_empty());
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.engine.has_node(a) && actor.engine.has_node(b));
}

#[test]
fn connection_commands_only_broadcast_actual_changes() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let exec = ConnectionModel {
        src_node: a,
        src_pin: PinId(4),
        dst_node: b,
        dst_pin: PinId(0),
    };
    let data = ConnectionModel {
        src_pin: PinId(5),
        dst_pin: PinId(1),
        ..exec
    };
    assert!(
        matches!(apply(&mut actor, ClientMessage::ConnectExec(exec))[0], ServerMessage::ExecConnected(c) if c == exec)
    );
    assert!(apply(&mut actor, ClientMessage::ConnectExec(exec)).is_empty());
    assert!(
        matches!(apply(&mut actor, ClientMessage::ConnectData(data))[0], ServerMessage::DataConnected(c) if c == data)
    );
    assert!(apply(&mut actor, ClientMessage::ConnectData(data)).is_empty());
    assert!(
        matches!(apply(&mut actor, ClientMessage::DisconnectExec(exec))[0], ServerMessage::ExecDisconnected(c) if c == exec)
    );
    assert!(apply(&mut actor, ClientMessage::DisconnectExec(exec)).is_empty());
    let disconnect = ClientMessage::DisconnectData {
        dst_node: b,
        dst_pin: PinId(1),
    };
    assert!(
        matches!(apply(&mut actor, disconnect.clone())[0], ServerMessage::DataDisconnected { dst_node, dst_pin } if dst_node == b && dst_pin == PinId(1))
    );
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
    snippet.data_connections.push(ConnectionModel {
        src_node: a,
        src_pin: PinId(99),
        dst_node: b,
        dst_pin: PinId(1),
    });
    // Invalid pins detach; a cycle remains a paste warning.
    snippet.data_connections.push(ConnectionModel {
        src_node: a,
        src_pin: PinId(5),
        dst_node: a,
        dst_pin: PinId(1),
    });
    let expected_positions = snippet
        .nodes
        .iter()
        .map(|node| node.position)
        .collect::<Vec<_>>();
    let (reply, mut replies) = mpsc::unbounded_channel();
    let (events, created) = actor
        .apply(
            ClientMessage::Paste(snippet.clone()),
            actor.sheets.first().map(|s| s.id),
            &reply,
        )
        .unwrap();
    let events: Vec<ServerMessage> = events.into_iter().map(|(_, m)| m).collect();
    let ids = created.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| *id != a && *id != b));
    let positions = events
        .iter()
        .filter_map(|event| match event {
            ServerMessage::NodeAdded(node) => Some(node.model.position),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(positions, expected_positions);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ServerMessage::Detached(_)))
    );
    let Ok(ServerMessage::Error(warning)) = replies.try_recv() else {
        panic!("missing paste warning")
    };
    assert_eq!(warning.key, "error-paste-incomplete");
    assert!(warning.fallback.contains("Data connection"));
    let node = snippet.nodes[0].clone();
    snippet.nodes = vec![node; MAX_PASTE_NODES + 1];
    let error = actor
        .apply(
            ClientMessage::Paste(snippet),
            actor.sheets.first().map(|s| s.id),
            &reply,
        )
        .unwrap_err();
    assert_eq!(error.key, "error-paste-too-many");
    assert_eq!(
        error.args,
        vec![
            ("count".into(), "1001".into()),
            ("max".into(), "1000".into())
        ]
    );
    assert_eq!(actor.engine.node_ids().len(), 4);
}

#[test]
fn history_batch_collects_created_ids_and_keeps_first_command_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let (reply, mut errors) = mpsc::unbounded_channel();
    let missing = NodeId::generate();
    let (_, ids) = actor
        .apply(
            ClientMessage::Batch(vec![
                ClientMessage::RemoveNode(missing),
                ClientMessage::Undo,
                ClientMessage::AddNode {
                    type_name: "const".into(),
                    position: Position::default(),
                },
                ClientMessage::AddNode {
                    type_name: "for_loop".into(),
                    position: Position::default(),
                },
            ]),
            actor.sheets.first().map(|s| s.id),
            &reply,
        )
        .unwrap();
    let ids = ids.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.iter().all(|id| actor.engine.has_node(*id)));
    assert!(
        matches!(errors.try_recv(), Ok(ServerMessage::Error(e)) if e == EngineError::NodeNotFound(missing).ui_message())
    );
    assert!(errors.try_recv().is_err());
    apply(&mut actor, ClientMessage::Undo);
    assert!(actor.engine.node_ids().is_empty());
}
