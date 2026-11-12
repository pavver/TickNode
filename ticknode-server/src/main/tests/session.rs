use super::*;

#[tokio::test]
async fn editor_session_over_websocket_persists_graph() {
    let dir = tempfile::tempdir().unwrap();
    let graph_path = dir.path().join("graph.json");
    let handle = actor::spawn(
        TickEngine::new(),
        Default::default(),
        Storage::new(graph_path.clone()),
        plugins::Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 10,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, router(handle.clone(), dir.path())).into_future());

    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr))
        .await
        .unwrap();
    let ServerMessage::Init {
        open_sheet, sheets, ..
    } = recv(&mut ws).await
    else {
        panic!("expected Init")
    };
    assert_eq!(open_sheet, None, "a new session starts on the home page");
    let sheet_id = sheets[0].info.id;
    send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
    let ServerMessage::Init {
        definitions,
        nodes,
        open_sheet,
        ..
    } = recv(&mut ws).await
    else {
        panic!("expected Init")
    };
    assert_eq!(open_sheet, Some(sheet_id));
    assert!(definitions.iter().any(|d| d.type_name == "const"));
    assert!(nodes.is_empty());

    send(
        &mut ws,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position { x: 1.0, y: 2.0 },
        },
    )
    .await;
    let ServerMessage::NodeAdded(c) = recv(&mut ws).await else {
        panic!()
    };
    // The initiator learns which node was created after NodeAdded
    assert_eq!(
        recv(&mut ws).await,
        ServerMessage::Created(vec![c.model.id])
    );
    send(
        &mut ws,
        ClientMessage::AddNode {
            type_name: "test/debug".into(),
            position: Position::default(),
        },
    )
    .await;
    let ServerMessage::NodeAdded(dbg) = recv(&mut ws).await else {
        panic!()
    };
    assert_eq!(
        recv(&mut ws).await,
        ServerMessage::Created(vec![dbg.model.id])
    );

    send(
        &mut ws,
        ClientMessage::SetProperty {
            id: c.model.id,
            name: "value".into(),
            value: DataValue::String("hi".into()),
        },
    )
    .await;
    let ServerMessage::NodeUpdated(updated) = recv(&mut ws).await else {
        panic!()
    };
    assert_eq!(
        updated.model.properties,
        vec![("value".to_string(), DataValue::String("hi".into()))]
    );

    let conn = ConnectionModel {
        src_node: c.model.id,
        src_pin: PinId(0),
        dst_node: dbg.model.id,
        dst_pin: PinId(1),
    };
    send(&mut ws, ClientMessage::ConnectData(conn)).await;
    assert_eq!(recv(&mut ws).await, ServerMessage::DataConnected(conn));

    // Invalid connection — error sent only to this client
    send(&mut ws, ClientMessage::ConnectExec(conn)).await;
    assert!(matches!(recv(&mut ws).await, ServerMessage::Error(_)));

    // Node trigger: Debug writes a log, and the server broadcasts the execution trace with inputs
    send(
        &mut ws,
        ClientMessage::TriggerNode {
            id: dbg.model.id,
            pin: PinId(0),
        },
    )
    .await;
    let (mut traced, mut logged) = (false, false);
    while !(traced && logged) {
        match recv_any(&mut ws).await {
            ServerMessage::Executed(traces) => {
                let t = traces.iter().find(|t| t.node_id == dbg.model.id).unwrap();
                assert_eq!(
                    t.inputs,
                    vec![(PinId(1), Some(DataValue::String("hi".into())))]
                );
                traced = true;
            }
            ServerMessage::Log { node_id, message } => {
                assert_eq!((node_id, message.as_str()), (dbg.model.id, "hi"));
                logged = true;
            }
            _ => {}
        }
    }

    // A newly connected editor receives the latest execution traces in its snapshot
    let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", addr))
        .await
        .unwrap();
    let ServerMessage::Init { .. } = recv(&mut ws2).await else {
        panic!("expected Init")
    };
    send(&mut ws2, ClientMessage::OpenSheet(Some(sheet_id))).await;
    let ServerMessage::Init { traces, .. } = recv(&mut ws2).await else {
        panic!("expected Init")
    };
    assert!(traces.iter().any(|(t, _)| t.node_id == dbg.model.id));

    // Paste a copy of both nodes with their connection: new IDs, preserved positions, Created sent only to initiator and after NodeAdded
    let mut snippet = ticknode_shared::GraphModel {
        nodes: vec![updated.model.clone(), dbg.model.clone()],
        exec_connections: vec![],
        data_connections: vec![conn],
        wire_routes: Vec::new(),
        detached_connections: Vec::new(),
        frames: Vec::new(),
    };
    snippet.nodes[0].position = Position { x: 100.0, y: 200.0 };
    send(&mut ws, ClientMessage::Paste(snippet)).await;
    let (mut pasted, mut added, mut connected) = (None, Vec::new(), None);
    while pasted.is_none() || added.len() < 2 || connected.is_none() {
        match recv_any(&mut ws).await {
            ServerMessage::Created(ids) => {
                assert_eq!(added.len(), 2, "Created before NodeAdded");
                pasted = Some(ids);
            }
            ServerMessage::NodeAdded(n) => added.push(n),
            ServerMessage::DataConnected(c) => connected = Some(c),
            _ => {}
        }
    }
    let ids = pasted.unwrap();
    assert_eq!(ids.len(), 2);
    assert!(!ids.contains(&c.model.id) && !ids.contains(&dbg.model.id));
    let copy = added.iter().find(|n| n.model.id == ids[0]).unwrap();
    assert_eq!(copy.model.position, Position { x: 100.0, y: 200.0 });
    assert_eq!(
        copy.model.properties,
        vec![("value".to_string(), DataValue::String("hi".into()))]
    );
    assert_eq!(
        connected.unwrap(),
        ConnectionModel {
            src_node: ids[0],
            src_pin: PinId(0),
            dst_node: ids[1],
            dst_pin: PinId(1)
        }
    );

    handle.shutdown().await;
    let sheet_file = dir.path().join("sheets").join(format!("{}.json", sheet_id));
    let saved: ticknode_shared::GraphModel =
        serde_json::from_str(&std::fs::read_to_string(sheet_file).unwrap()).unwrap();
    assert_eq!(saved.nodes.len(), 4);
    assert_eq!(saved.data_connections.len(), 2);
    assert!(saved.data_connections.contains(&conn));
    let saved_const = saved.nodes.iter().find(|n| n.id == c.model.id).unwrap();
    assert_eq!(saved_const.position, Position { x: 1.0, y: 2.0 });
}
