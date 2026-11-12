use super::*;
use std::time::Duration;

fn actor(dir: &std::path::Path) -> Actor {
    test_actor(dir)
}

fn apply(actor: &mut Actor, msg: ClientMessage, sheet: Option<SheetId>) -> Vec<ServerMessage> {
    let (reply, _) = mpsc::unbounded_channel();
    actor
        .apply(msg, sheet, &reply)
        .unwrap()
        .0
        .into_iter()
        .map(|(_, m)| m)
        .collect()
}

fn apply_err(actor: &mut Actor, msg: ClientMessage, sheet: Option<SheetId>) -> UiMessage {
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, sheet, &reply).unwrap_err()
}

#[test]
fn cross_sheet_connections_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet1 = actor.sheets[0].id;

    // Create a second sheet
    let (reply, mut replies) = mpsc::unbounded_channel();
    actor
        .apply(
            ClientMessage::CreateSheet {
                title: "Sheet 2".into(),
                caption: "".into(),
            },
            Some(sheet1),
            &reply,
        )
        .unwrap();
    let sheet2 = match replies.try_recv().unwrap() {
        ServerMessage::SheetCreated(id) => id,
        other => panic!("expected SheetCreated, got {other:?}"),
    };

    // Add node on Sheet 1
    let node1 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "for_loop".into(),
            position: Position::default(),
        },
        Some(sheet1),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    // Add node on Sheet 2
    let node2 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "for_loop".into(),
            position: Position::default(),
        },
        Some(sheet2),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    // Cross-sheet connect data must fail with error-cross-sheet
    let conn = ConnectionModel {
        src_node: node1,
        src_pin: PinId(5),
        dst_node: node2,
        dst_pin: PinId(1),
    };
    let err = apply_err(&mut actor, ClientMessage::ConnectData(conn), Some(sheet1));
    assert_eq!(err.key, "error-cross-sheet");

    // Cross-sheet connect exec must also fail
    let exec_conn = ConnectionModel {
        src_node: node1,
        src_pin: PinId(4),
        dst_node: node2,
        dst_pin: PinId(0),
    };
    let err = apply_err(
        &mut actor,
        ClientMessage::ConnectExec(exec_conn),
        Some(sheet1),
    );
    assert_eq!(err.key, "error-cross-sheet");
}

#[test]
fn per_sheet_undo_does_not_affect_other_sheets() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet1 = actor.sheets[0].id;

    let (reply, mut replies) = mpsc::unbounded_channel();
    actor
        .apply(
            ClientMessage::CreateSheet {
                title: "Sheet 2".into(),
                caption: "".into(),
            },
            Some(sheet1),
            &reply,
        )
        .unwrap();
    let sheet2 = match replies.try_recv().unwrap() {
        ServerMessage::SheetCreated(id) => id,
        _ => panic!(),
    };

    let node1 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position::default(),
        },
        Some(sheet1),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    let node2 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "for_loop".into(),
            position: Position::default(),
        },
        Some(sheet2),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    assert!(actor.engine.has_node(node1));
    assert!(actor.engine.has_node(node2));

    // Undo on Sheet 2 removes node2, but leaves node1 intact on Sheet 1
    let events = apply(&mut actor, ClientMessage::Undo, Some(sheet2));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::NodeRemoved(id) if *id == node2))
    );
    assert!(actor.engine.has_node(node1));
    assert!(!actor.engine.has_node(node2));

    // Redo on Sheet 2 restores node2
    let events = apply(&mut actor, ClientMessage::Redo, Some(sheet2));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::NodeAdded(n) if n.model.id == node2))
    );
    assert!(actor.engine.has_node(node1));
    assert!(actor.engine.has_node(node2));

    // Undo on Sheet 1 removes node1, leaving node2 on Sheet 2
    let events = apply(&mut actor, ClientMessage::Undo, Some(sheet1));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::NodeRemoved(id) if *id == node1))
    );
    assert!(!actor.engine.has_node(node1));
    assert!(actor.engine.has_node(node2));
}

#[test]
fn disabled_sheet_nodes_are_marked_disabled_and_do_not_execute() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet1 = actor.sheets[0].id;

    let id = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "for_loop".into(),
            position: Position::default(),
        },
        Some(sheet1),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    assert!(!actor.engine.is_disabled(id));

    // Disable the sheet
    let info = SheetInfo {
        id: sheet1,
        title: "Main".into(),
        caption: "".into(),
        enabled: false,
    };
    apply(&mut actor, ClientMessage::UpdateSheet(info), Some(sheet1));

    assert!(actor.engine.is_disabled(id));

    // Trigger node on disabled sheet: trigger does not schedule or execute it
    let (reply, _) = mpsc::unbounded_channel();
    actor
        .apply(
            ClientMessage::TriggerNode { id, pin: PinId(0) },
            Some(sheet1),
            &reply,
        )
        .unwrap();
    actor.engine.tick();
    assert!(!actor.engine.executed.iter().any(|t| t.node_id == id));

    // Re-enabling the sheet re-enables the nodes
    let info = SheetInfo {
        id: sheet1,
        title: "Main".into(),
        caption: "".into(),
        enabled: true,
    };
    apply(&mut actor, ClientMessage::UpdateSheet(info), Some(sheet1));
    assert!(!actor.engine.is_disabled(id));
}

#[test]
fn duplicate_sheet_clones_content_with_new_ids() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet1 = actor.sheets[0].id;

    let node1 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position { x: 10.0, y: 20.0 },
        },
        Some(sheet1),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    let (reply, mut replies) = mpsc::unbounded_channel();
    actor
        .apply(ClientMessage::DuplicateSheet(sheet1), Some(sheet1), &reply)
        .unwrap();
    let sheet2 = match replies.try_recv().unwrap() {
        ServerMessage::SheetCreated(id) => id,
        _ => panic!(),
    };

    assert_ne!(sheet1, sheet2);
    let s2_info = actor.sheets.iter().find(|s| s.id == sheet2).unwrap();
    assert_eq!(s2_info.title, "Main (copy)");

    let s2_graph = actor.sheet_graph(sheet2);
    assert_eq!(s2_graph.nodes.len(), 1);
    assert_ne!(s2_graph.nodes[0].id, node1);
    assert_eq!(s2_graph.nodes[0].position, Position { x: 10.0, y: 20.0 });
    assert_eq!(s2_graph.nodes[0].type_name, "const");
}

#[test]
fn delete_sheet_moves_file_to_trash_and_removes_nodes() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet1 = actor.sheets[0].id;

    let (reply, mut replies) = mpsc::unbounded_channel();
    actor
        .apply(
            ClientMessage::CreateSheet {
                title: "To Delete".into(),
                caption: "".into(),
            },
            Some(sheet1),
            &reply,
        )
        .unwrap();
    let sheet2 = match replies.try_recv().unwrap() {
        ServerMessage::SheetCreated(id) => id,
        _ => panic!(),
    };

    let node2 = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position::default(),
        },
        Some(sheet2),
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();

    actor.save();
    assert!(actor.storage.sheet_path(sheet2).exists());
    assert!(actor.engine.has_node(node2));

    // Delete Sheet 2
    apply(&mut actor, ClientMessage::DeleteSheet(sheet2), Some(sheet1));

    assert!(!actor.engine.has_node(node2));
    assert!(!actor.sheets.iter().any(|s| s.id == sheet2));
    assert!(!actor.storage.sheet_path(sheet2).exists());

    let trash_files = std::fs::read_dir(actor.storage.trash_dir())
        .unwrap()
        .collect::<Vec<_>>();
    assert_eq!(trash_files.len(), 1);
}

#[tokio::test]
async fn event_filtering_by_session_sheet_over_websocket() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let storage = Storage::new(dir.path());
    let loaded = storage.load(&mut engine).unwrap();
    let sheet1 = loaded.sheets[0].id;

    let handle = spawn(
        engine,
        loaded,
        storage,
        Plugins::new(dir.path()),
        ServerSettings {
            tick_ms: 10,
            tick_budget: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(axum::serve(listener, crate::router(handle.clone(), dir.path())).into_future());

    // Client 1 connects on the home page, then opens sheet1
    let (mut ws1, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();
    let ServerMessage::Init {
        open_sheet: s1_open,
        ..
    } = recv(&mut ws1).await
    else {
        panic!()
    };
    assert_eq!(s1_open, None);
    send(&mut ws1, ClientMessage::OpenSheet(Some(sheet1))).await;
    let ServerMessage::Init {
        open_sheet: s1_open,
        ..
    } = recv(&mut ws1).await
    else {
        panic!()
    };
    assert_eq!(s1_open, Some(sheet1));

    // Client 2 connects, creates Sheet 2, and opens it
    let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/ws"))
        .await
        .unwrap();
    let ServerMessage::Init { .. } = recv(&mut ws2).await else {
        panic!()
    };

    send(
        &mut ws2,
        ClientMessage::CreateSheet {
            title: "Sheet 2".into(),
            caption: "".into(),
        },
    )
    .await;
    let sheet2 = match recv(&mut ws2).await {
        ServerMessage::SheetCreated(id) => id,
        other => panic!("expected SheetCreated, got {other:?}"),
    };

    // Client 2 opens Sheet 2
    send(&mut ws2, ClientMessage::OpenSheet(Some(sheet2))).await;
    let ServerMessage::Init {
        open_sheet: s2_open,
        ..
    } = recv(&mut ws2).await
    else {
        panic!()
    };
    assert_eq!(s2_open, Some(sheet2));

    // Client 1 adds a node on Sheet 1
    send(
        &mut ws1,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position { x: 50.0, y: 50.0 },
        },
    )
    .await;

    // Client 1 receives NodeAdded
    let node_msg = recv(&mut ws1).await;
    assert!(matches!(node_msg, ServerMessage::NodeAdded(_)));

    // Client 2 should NOT receive the NodeAdded for Sheet 1!
    // Try receiving on ws2 with a short timeout
    let ws2_result = tokio::time::timeout(Duration::from_millis(150), recv_any(&mut ws2)).await;
    match ws2_result {
        Ok(msg) => panic!("Client 2 on Sheet 2 received unexpected message: {msg:?}"),
        Err(_) => {} // Timeout as expected!
    }

    handle.shutdown().await;
}

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn send(ws: &mut Socket, msg: ClientMessage) {
    ws.send(Message::Binary(postcard::to_allocvec(&msg).unwrap().into()))
        .await
        .unwrap();
}

async fn recv(ws: &mut Socket) -> ServerMessage {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Binary(b) = msg {
            let msg: ServerMessage = postcard::from_bytes(&b).unwrap();
            if !matches!(
                msg,
                ServerMessage::Executed(_)
                    | ServerMessage::Log { .. }
                    | ServerMessage::PropertyOptions { .. }
                    | ServerMessage::History(_)
                    | ServerMessage::Sheets(_)
            ) {
                return msg;
            }
        }
    }
}

async fn recv_any(ws: &mut Socket) -> ServerMessage {
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let Message::Binary(b) = msg {
            return postcard::from_bytes(&b).unwrap();
        }
    }
}
