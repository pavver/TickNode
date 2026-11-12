use super::*;
use ticknode_shared::{Frame, FrameColor, FrameId, MAX_FRAMES};

fn actor(dir: &std::path::Path) -> Actor {
    test_actor(dir)
}

fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    test_apply(actor, msg)
}

fn apply_err(actor: &mut Actor, msg: ClientMessage) -> UiMessage {
    let sheet = actor.sheets.first().map(|s| s.id);
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, sheet, &reply).unwrap_err()
}

fn history_state(actor: &Actor) -> HistoryState {
    let sheet_id = actor.sheets[0].id;
    actor.histories.get(&sheet_id).map_or_default(|h| h.state())
}

fn sample_frame(id: FrameId, title: &str) -> Frame {
    Frame {
        id,
        title: title.into(),
        color: FrameColor::Blue,
        position: Position { x: 50.0, y: 70.0 },
        size: NodeSize {
            width: 320.0,
            height: 240.0,
        },
    }
}

#[test]
fn set_and_persist_after_reload() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    let frame = sample_frame(id, "Comment 1");
    let command = ClientMessage::SetFrame(frame.clone());
    let decoded = postcard::from_bytes(&postcard::to_allocvec(&command).unwrap()).unwrap();
    let events = apply(&mut actor, decoded);
    assert_eq!(events[0], ServerMessage::FrameSet(frame.clone()));
    assert_eq!(
        postcard::from_bytes::<ServerMessage>(&encode(&events[0])).unwrap(),
        events[0]
    );
    assert_eq!(actor.frames.get(&id), Some(&frame));

    actor.save();
    let sheet_id = actor.sheets[0].id;
    let first = std::fs::read_to_string(actor.storage.sheet_path(sheet_id)).unwrap();
    let graph: GraphModel = serde_json::from_str(&first).unwrap();
    assert_eq!(graph.frames, vec![frame.clone()]);

    let mut engine = TickEngine::new();
    let loaded = actor.storage.load(&mut engine).unwrap();
    assert_eq!(loaded.frames.get(&id), Some(&frame));
    actor.storage.save_sheet(sheet_id, &graph).unwrap();
    assert_eq!(
        std::fs::read_to_string(actor.storage.sheet_path(sheet_id)).unwrap(),
        first
    );
}

#[test]
fn replace_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    let frame1 = sample_frame(id, "Initial");
    apply(&mut actor, ClientMessage::SetFrame(frame1));

    let mut frame2 = sample_frame(id, "Updated");
    frame2.color = FrameColor::Green;
    frame2.position = Position { x: 100.0, y: 150.0 };
    frame2.size = NodeSize {
        width: 400.0,
        height: 300.0,
    };

    let events = apply(&mut actor, ClientMessage::SetFrame(frame2.clone()));
    assert_eq!(events[0], ServerMessage::FrameSet(frame2.clone()));
    assert_eq!(actor.frames.get(&id), Some(&frame2));
}

#[test]
fn noop_produces_no_event_or_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    let frame = sample_frame(id, "Fixed");
    apply(&mut actor, ClientMessage::SetFrame(frame.clone()));
    let history_before = history_state(&actor);

    // Identical frame produces empty events and no history entry
    let events = apply(&mut actor, ClientMessage::SetFrame(frame.clone()));
    assert!(events.is_empty());
    assert_eq!(history_state(&actor), history_before);

    // Removing unknown frame produces empty events and no history entry
    let unknown_id = FrameId::generate();
    let events = apply(&mut actor, ClientMessage::RemoveFrame(unknown_id));
    assert!(events.is_empty());
    assert_eq!(history_state(&actor), history_before);
}

#[test]
fn invalid_frame_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();

    let nonfinite_pos = Frame {
        position: Position {
            x: f32::NAN,
            y: 0.0,
        },
        ..sample_frame(id, "NanPos")
    };
    let err = apply_err(&mut actor, ClientMessage::SetFrame(nonfinite_pos));
    assert_eq!(err.key, "error-frame-invalid");

    let nonfinite_size = Frame {
        size: NodeSize {
            width: f64::INFINITY,
            height: 100.0,
        },
        ..sample_frame(id, "InfSize")
    };
    let err = apply_err(&mut actor, ClientMessage::SetFrame(nonfinite_size));
    assert_eq!(err.key, "error-frame-invalid");
    assert!(actor.frames.is_empty());
}

#[test]
fn frame_limit_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());

    for i in 0..MAX_FRAMES {
        let frame = sample_frame(FrameId::generate(), &format!("Frame {i}"));
        actor.frames.insert(frame.id, frame);
    }
    assert_eq!(actor.frames.len(), MAX_FRAMES);

    // Adding one more fails
    let new_frame = sample_frame(FrameId::generate(), "Over limit");
    let err = apply_err(&mut actor, ClientMessage::SetFrame(new_frame));
    assert_eq!(err.key, "error-frame-limit");

    // But replacing an existing frame still succeeds
    let existing_id = *actor.frames.keys().next().unwrap();
    let mut updated = sample_frame(existing_id, "Updated in full store");
    updated.color = FrameColor::Purple;
    let events = apply(&mut actor, ClientMessage::SetFrame(updated.clone()));
    assert_eq!(events[0], ServerMessage::FrameSet(updated.clone()));
    assert_eq!(actor.frames.get(&existing_id), Some(&updated));
}

#[test]
fn remove_frame() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    apply(
        &mut actor,
        ClientMessage::SetFrame(sample_frame(id, "To Remove")),
    );
    assert!(actor.frames.contains_key(&id));

    let events = apply(&mut actor, ClientMessage::RemoveFrame(id));
    assert_eq!(events[0], ServerMessage::FrameRemoved(id));
    assert!(!actor.frames.contains_key(&id));
}

#[test]
fn undo_redo_of_set_and_remove() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    let frame = sample_frame(id, "History Test");

    apply(&mut actor, ClientMessage::SetFrame(frame.clone()));
    assert_eq!(actor.frames.get(&id), Some(&frame));

    // Undo SetFrame
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(events[0], ServerMessage::FrameRemoved(id));
    assert!(!actor.frames.contains_key(&id));

    // Redo SetFrame
    let events = apply(&mut actor, ClientMessage::Redo);
    assert_eq!(events[0], ServerMessage::FrameSet(frame.clone()));
    assert_eq!(actor.frames.get(&id), Some(&frame));

    // Remove frame
    apply(&mut actor, ClientMessage::RemoveFrame(id));
    assert!(!actor.frames.contains_key(&id));

    // Undo RemoveFrame
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(events[0], ServerMessage::FrameSet(frame.clone()));
    assert_eq!(actor.frames.get(&id), Some(&frame));

    // Redo RemoveFrame
    let events = apply(&mut actor, ClientMessage::Redo);
    assert_eq!(events[0], ServerMessage::FrameRemoved(id));
    assert!(!actor.frames.contains_key(&id));
}

#[test]
fn batch_set_frame_and_move_node_is_one_history_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let node_id = apply(
        &mut actor,
        ClientMessage::AddNode {
            type_name: "const".into(),
            position: Position { x: 10.0, y: 10.0 },
        },
    )
    .into_iter()
    .find_map(|e| match e {
        ServerMessage::NodeAdded(n) => Some(n.model.id),
        _ => None,
    })
    .unwrap();
    actor.histories.clear();

    let frame_id = FrameId::generate();
    let frame = sample_frame(frame_id, "Batch Frame");
    let batch = ClientMessage::Batch(vec![
        ClientMessage::SetFrame(frame.clone()),
        ClientMessage::MoveNode {
            id: node_id,
            position: Position { x: 50.0, y: 50.0 },
        },
    ]);
    let events = apply(&mut actor, batch);
    assert!(events.contains(&ServerMessage::FrameSet(frame.clone())));
    assert!(events.contains(&ServerMessage::NodeMoved {
        id: node_id,
        position: Position { x: 50.0, y: 50.0 }
    }));

    // One undo restores both
    let events = apply(&mut actor, ClientMessage::Undo);
    assert!(events.contains(&ServerMessage::FrameRemoved(frame_id)));
    assert!(events.contains(&ServerMessage::NodeMoved {
        id: node_id,
        position: Position { x: 10.0, y: 10.0 }
    }));
    assert!(!actor.frames.contains_key(&frame_id));

    // One redo restores both
    let events = apply(&mut actor, ClientMessage::Redo);
    assert!(events.contains(&ServerMessage::FrameSet(frame.clone())));
    assert!(events.contains(&ServerMessage::NodeMoved {
        id: node_id,
        position: Position { x: 50.0, y: 50.0 }
    }));
    assert!(actor.frames.contains_key(&frame_id));
}

#[test]
fn paste_gives_new_ids_and_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let frame_id = FrameId::generate();
    let frame = sample_frame(frame_id, "Original");
    apply(&mut actor, ClientMessage::SetFrame(frame.clone()));

    let mut snippet = actor.graph_snapshot();
    // Offset the frame in the snippet
    snippet.frames[0].position = Position { x: 150.0, y: 170.0 };

    let events = apply(&mut actor, ClientMessage::Paste(snippet));
    let pasted_frame = events
        .iter()
        .find_map(|e| match e {
            ServerMessage::FrameSet(f) => Some(f.clone()),
            _ => None,
        })
        .unwrap();

    assert_ne!(pasted_frame.id, frame_id);
    assert_eq!(pasted_frame.position, Position { x: 150.0, y: 170.0 });
    assert_eq!(pasted_frame.title, "Original");
    assert_eq!(actor.frames.len(), 2);

    // Undo paste removes the pasted frame
    let events = apply(&mut actor, ClientMessage::Undo);
    assert!(events.contains(&ServerMessage::FrameRemoved(pasted_frame.id)));
    assert_eq!(actor.frames.len(), 1);
    assert!(actor.frames.contains_key(&frame_id));
}

#[test]
fn init_carries_frames() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = FrameId::generate();
    let frame = sample_frame(id, "Init Frame");
    apply(&mut actor, ClientMessage::SetFrame(frame.clone()));

    let snapshot = actor.snapshot(Some(actor.sheets[0].id));
    let ServerMessage::Init { frames, .. } = &snapshot else {
        panic!("Expected Init")
    };
    assert_eq!(frames, &vec![frame.clone()]);

    let decoded: ServerMessage = postcard::from_bytes(&encode(&snapshot)).unwrap();
    let ServerMessage::Init {
        frames: decoded_frames,
        ..
    } = decoded
    else {
        panic!("Expected Init")
    };
    assert_eq!(decoded_frames, vec![frame]);
}
