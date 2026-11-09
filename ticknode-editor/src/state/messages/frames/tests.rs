use super::*;
use ticknode_shared::*;

fn test_frame(id: FrameId, title: &str) -> Frame {
    Frame {
        id,
        title: title.to_string(),
        color: FrameColor::Gray,
        position: Position { x: 100.0, y: 100.0 },
        size: NodeSize { width: 400.0, height: 240.0 },
    }
}

fn apply_twice(all: &mut BTreeMap<FrameId, Frame>, event: ServerMessage) {
    assert!(applies(&event));
    apply(all, &event);
    let after = all.clone();
    apply(all, &event);
    assert_eq!(*all, after);
}

#[test]
fn frame_set_and_remove_are_idempotent() {
    let mut all = BTreeMap::new();
    let id1 = FrameId::generate();
    let id2 = FrameId::generate();

    let f1 = test_frame(id1, "First");
    let f2 = test_frame(id2, "Second");

    apply_twice(&mut all, ServerMessage::FrameSet(f1.clone()));
    assert_eq!(all.len(), 1);
    assert_eq!(all.get(&id1).unwrap().title, "First");

    // Update existing frame
    let mut f1_updated = f1.clone();
    f1_updated.title = "Renamed".to_string();
    f1_updated.color = FrameColor::Blue;
    apply_twice(&mut all, ServerMessage::FrameSet(f1_updated));
    assert_eq!(all.len(), 1);
    assert_eq!(all.get(&id1).unwrap().title, "Renamed");
    assert_eq!(all.get(&id1).unwrap().color, FrameColor::Blue);

    // Insert second frame
    apply_twice(&mut all, ServerMessage::FrameSet(f2));
    assert_eq!(all.len(), 2);

    // Remove first frame
    apply_twice(&mut all, ServerMessage::FrameRemoved(id1));
    assert_eq!(all.len(), 1);
    assert!(!all.contains_key(&id1));
    assert!(all.contains_key(&id2));

    // Remove non-existent is a no-op and idempotent
    apply_twice(&mut all, ServerMessage::FrameRemoved(id1));
    assert_eq!(all.len(), 1);

    // Remove second frame
    apply_twice(&mut all, ServerMessage::FrameRemoved(id2));
    assert!(all.is_empty());
}

#[test]
fn init_replaces_all_frames_including_empty_snapshot() {
    let mut all = BTreeMap::new();
    let id1 = FrameId::generate();
    let id2 = FrameId::generate();

    let snapshot = |frames| ServerMessage::Init {
        definitions: vec![],
        nodes: vec![],
        exec_connections: vec![],
        data_connections: vec![],
        traces: vec![],
        types: vec![],
        plugins: vec![],
        translations: vec![],
        property_options: vec![],
        history: HistoryState::default(),
        wire_routes: vec![],
        detached_connections: vec![],
        timers: vec![],
        node_errors: vec![],
        frames,
        sheets: vec![],
        open_sheet: None,
    };

    apply_twice(&mut all, snapshot(vec![test_frame(id1, "One"), test_frame(id2, "Two")]));
    assert_eq!(all.len(), 2);
    assert!(all.contains_key(&id1));
    assert!(all.contains_key(&id2));

    apply_twice(&mut all, snapshot(vec![test_frame(id1, "Only One")]));
    assert_eq!(all.len(), 1);
    assert_eq!(all.get(&id1).unwrap().title, "Only One");

    apply_twice(&mut all, snapshot(vec![]));
    assert!(all.is_empty());

    assert!(!applies(&ServerMessage::History(HistoryState::default())));
}
