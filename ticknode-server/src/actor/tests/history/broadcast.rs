use super::*;

#[tokio::test]
async fn actor_ticks_broadcast_logs_and_keep_traces_in_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "console", &[]).unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::from("tick output")))
        .unwrap();
    let storage = Storage::new(dir.path());
    let sheet_id = SheetId::generate();
    let loaded = LoadedSheets {
        sheets: vec![SheetInfo::new(sheet_id, "Main")],
        node_sheet: HashMap::from([(id, sheet_id)]),
        frame_sheet: HashMap::new(),
        positions: HashMap::new(),
        sizes: HashMap::new(),
        routes: HashMap::new(),
        frames: BTreeMap::new(),
    };
    let first_sheet = loaded.sheets.first().map(|s| s.id);
    let handle = spawn(
        engine,
        loaded,
        storage,
        Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 5,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut events = handle.subscribe();
    let (reply, _) = mpsc::unbounded_channel();
    handle.send(
        ClientMessage::TriggerNode { id, pin: PinId(0) },
        None,
        reply,
    );
    let mut executed = false;
    let mut logged = false;
    while !executed || !logged {
        let (_sheet, bytes) = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .unwrap()
            .unwrap();
        match postcard::from_bytes::<ServerMessage>(&bytes).unwrap() {
            ServerMessage::Executed(traces) => {
                assert!(traces.iter().any(|trace| {
                    trace.node_id == id
                        && trace
                            .inputs
                            .contains(&(PinId(1), Some(DataValue::from("tick output"))))
                }));
                executed = true;
            }
            ServerMessage::Log { node_id, message } => {
                assert_eq!((node_id, message.as_str()), (id, "tick output"));
                logged = true;
            }
            _ => {}
        }
    }
    let Some(ServerMessage::Init { traces, .. }) = handle.snapshot(first_sheet).await else {
        panic!("missing snapshot")
    };
    assert!(traces.iter().any(|(trace, _)| trace.node_id == id));
    handle.shutdown().await;
}

#[tokio::test]
async fn history_is_broadcast_to_all_editors_and_in_new_snapshots() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let storage = Storage::new(dir.path());
    let loaded = storage.load(&mut engine).unwrap();
    let first_sheet = loaded.sheets.first().map(|s| s.id);
    let handle = spawn(
        engine,
        loaded,
        storage,
        Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 10_000,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut first = handle.subscribe();
    let mut second = handle.subscribe();
    let (reply, mut replies) = mpsc::unbounded_channel();
    handle.send(
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position::default(),
        },
        None,
        reply,
    );
    for receiver in [&mut first, &mut second] {
        let (_sheet, node) = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            postcard::from_bytes::<ServerMessage>(&node).unwrap(),
            ServerMessage::NodeAdded(_)
        ));
        let (_sheet, history) = tokio::time::timeout(Duration::from_secs(5), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            postcard::from_bytes::<ServerMessage>(&history).unwrap(),
            ServerMessage::History(HistoryState {
                can_undo: true,
                can_redo: false
            })
        );
    }
    assert!(matches!(
        replies.recv().await,
        Some(ServerMessage::Created(_))
    ));
    assert!(matches!(
        handle.snapshot(first_sheet).await,
        Some(ServerMessage::Init {
            history: HistoryState {
                can_undo: true,
                can_redo: false
            },
            ..
        })
    ));
    handle.shutdown().await;
}

#[tokio::test]
async fn add_node_with_creates_node_with_properties_replies_created_and_supports_undo() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let storage = Storage::new(dir.path());
    let loaded = storage.load(&mut engine).unwrap();
    let first_sheet = loaded.sheets.first().map(|s| s.id);
    let handle = spawn(
        engine,
        loaded,
        storage,
        Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 10_000,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let (reply, mut replies) = mpsc::unbounded_channel();
    let pos = Position { x: 42.0, y: 84.0 };
    handle.send(
        ClientMessage::AddNodeWith {
            type_name: "const".into(),
            position: pos,
            properties: vec![("value".into(), DataValue::from(123.0))],
        },
        None,
        reply,
    );

    let created_id = match replies.recv().await {
        Some(ServerMessage::Created(ids)) => {
            assert_eq!(ids.len(), 1);
            ids[0]
        }
        other => panic!("Expected Created, got {other:?}"),
    };

    let snapshot = handle.snapshot(first_sheet).await.unwrap();
    let ServerMessage::Init { nodes, .. } = snapshot else {
        panic!("Expected Init")
    };
    let node = nodes
        .iter()
        .find(|n| n.model.id == created_id)
        .expect("node exists");
    assert_eq!(node.model.position, pos);
    assert_eq!(
        node.model.properties,
        vec![("value".into(), DataValue::from(123.0))]
    );

    // Undo removes it
    let (undo_reply, _) = mpsc::unbounded_channel();
    handle.send(ClientMessage::Undo, None, undo_reply);
    tokio::time::sleep(Duration::from_millis(50)).await;

    let snapshot = handle.snapshot(first_sheet).await.unwrap();
    let ServerMessage::Init { nodes, .. } = snapshot else {
        panic!("Expected Init")
    };
    assert!(!nodes.iter().any(|n| n.model.id == created_id));

    // Rejects in:* and split:*
    let (err_reply, mut err_replies) = mpsc::unbounded_channel();
    handle.send(
        ClientMessage::AddNodeWith {
            type_name: "const".into(),
            position: Position::default(),
            properties: vec![("in:0".into(), DataValue::Bool(true))],
        },
        None,
        err_reply,
    );
    match err_replies.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-invalid-property");
            assert_eq!(err.args, vec![("property".into(), "in:0".into())]);
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    let (err_reply2, mut err_replies2) = mpsc::unbounded_channel();
    handle.send(
        ClientMessage::AddNodeWith {
            type_name: "const".into(),
            position: Position::default(),
            properties: vec![("split:0".into(), DataValue::Bool(true))],
        },
        None,
        err_reply2,
    );
    match err_replies2.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-invalid-property");
            assert_eq!(err.args, vec![("property".into(), "split:0".into())]);
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    // Unknown node type returns error
    let (err_reply3, mut err_replies3) = mpsc::unbounded_channel();
    handle.send(
        ClientMessage::AddNodeWith {
            type_name: "unknown_type".into(),
            position: Position::default(),
            properties: vec![],
        },
        None,
        err_reply3,
    );
    match err_replies3.recv().await {
        Some(ServerMessage::Error(err)) => {
            assert_eq!(err.key, "error-unknown-node-type");
        }
        other => panic!("Expected Error, got {other:?}"),
    }

    handle.shutdown().await;
}
