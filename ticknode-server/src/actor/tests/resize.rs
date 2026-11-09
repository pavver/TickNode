use super::*;

fn actor(dir: &std::path::Path) -> Actor {
    let (events, _) = broadcast::channel(32);
    Actor {
        engine: TickEngine::new(), positions: HashMap::new(), sizes: HashMap::new(), routes: HashMap::new(), frames: BTreeMap::new(),
        storage: Storage::new(dir.join("graph.json")), plugins: Plugins::new(dir), events,
        dirty_since: None, traces: HashMap::new(), history: Default::default(),
        telemetry: super::super::telemetry::Telemetry::new(Instant::now()),
    }
}
fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, &reply).unwrap().0
}
fn add(actor: &mut Actor) -> NodeId {
    let id = apply(actor, ClientMessage::AddNode { type_name: "const".into(), position: Position::default() })
        .into_iter().find_map(|e| match e { ServerMessage::NodeAdded(n) => Some(n.model.id), _ => None }).unwrap();
    actor.history = Default::default();
    actor.dirty_since = None;
    id
}
fn size(width: f64, height: f64) -> Option<NodeSize> { Some(NodeSize { width, height }) }

#[test]
fn resize_persists_in_init_and_json_without_changing_the_engine() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = add(&mut actor);
    let before = actor.engine.node_model(id);
    let dimensions = size(424.0, 252.0);
    let command = ClientMessage::ResizeNode { id, size: dimensions };
    let decoded = postcard::from_bytes(&postcard::to_allocvec(&command).unwrap()).unwrap();
    let events = apply(&mut actor, decoded);
    assert_eq!(events[0], ServerMessage::NodeResized { id, size: dimensions });
    assert_eq!(postcard::from_bytes::<ServerMessage>(&encode(&events[0])).unwrap(), events[0]);
    assert_eq!(actor.engine.node_model(id), before);
    let snapshot = actor.snapshot();
    assert_eq!(postcard::from_bytes::<ServerMessage>(&encode(&snapshot)).unwrap(), snapshot);
    let ServerMessage::Init { nodes, .. } = snapshot else { panic!() };
    assert_eq!(nodes[0].model.size, dimensions);
    actor.save();
    let first = std::fs::read_to_string(dir.path().join("graph.json")).unwrap();
    let graph: GraphModel = serde_json::from_str(&first).unwrap();
    assert_eq!(graph.nodes[0].size, dimensions);
    let mut engine = TickEngine::new();
    let (positions, sizes, routes, frames) = actor.storage.load(&mut engine).unwrap();
    assert_eq!(sizes[&id], dimensions.unwrap());
    actor.storage.save(&engine, &positions, &sizes, &routes, &frames).unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("graph.json")).unwrap(), first);
}

#[test]
fn resize_clamps_rejects_nonfinite_and_does_not_record_noops() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = add(&mut actor);
    assert!(apply(&mut actor, ClientMessage::ResizeNode { id, size: None }).is_empty());
    assert!(actor.dirty_since.is_none());
    assert!(!actor.history.state().can_undo);
    apply(&mut actor, ClientMessage::ResizeNode { id, size: size(-10.0, 2000.0) });
    assert_eq!(actor.sizes[&id], NodeSize { width: MIN_NODE_W, height: MAX_NODE_H });
    assert!(apply(&mut actor, ClientMessage::ResizeNode { id, size: size(0.0, 9999.0) }).is_empty());
    apply(&mut actor, ClientMessage::ResizeNode { id, size: size(10000.0, -1.0) });
    assert_eq!(actor.sizes[&id], NodeSize { width: MAX_NODE_W, height: 0.0 });
    actor.history = Default::default();
    actor.dirty_since = None;
    let (reply, _) = mpsc::unbounded_channel();
    for dimensions in [size(f64::NAN, 1.0), size(1.0, f64::INFINITY), size(f64::NEG_INFINITY, 0.0)] {
        let error = actor.apply(ClientMessage::ResizeNode { id, size: dimensions }, &reply).unwrap_err();
        assert_eq!(error.key, "error-node-size");
    }
    assert!(actor.apply(ClientMessage::ResizeNode { id: NodeId::generate(), size: None }, &reply).is_err());
    assert!(!actor.history.state().can_undo);
    assert!(actor.dirty_since.is_none());
}

#[test]
fn resize_batch_undo_redo_reset_and_removal_preserve_layout_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor);
    let b = add(&mut actor);
    apply(&mut actor, ClientMessage::Batch(vec![
        ClientMessage::ResizeNode { id: a, size: size(300.0, 200.0) },
        ClientMessage::ResizeNode { id: b, size: size(400.0, 300.0) },
    ]));
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(events.iter().filter(|e| matches!(e, ServerMessage::NodeResized { size: None, .. })).count(), 2);
    assert!(!events.iter().any(|e| matches!(e, ServerMessage::NodeUpdated(_) | ServerMessage::NodeAdded(_) | ServerMessage::NodeRemoved(_))));
    assert!(actor.sizes.is_empty());
    assert!(!actor.history.state().can_undo);
    apply(&mut actor, ClientMessage::Redo);
    assert_eq!(actor.sizes.get(&a).copied(), size(300.0, 200.0));
    assert_eq!(actor.sizes.get(&b).copied(), size(400.0, 300.0));
    // A no-op must retain redo.
    apply(&mut actor, ClientMessage::Undo);
    assert!(apply(&mut actor, ClientMessage::ResizeNode { id: a, size: None }).is_empty());
    assert!(actor.history.state().can_redo);
    apply(&mut actor, ClientMessage::Redo);
    apply(&mut actor, ClientMessage::ResizeNode { id: a, size: None });
    assert!(!actor.sizes.contains_key(&a));
    apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.sizes.get(&a).copied(), size(300.0, 200.0));
    apply(&mut actor, ClientMessage::RemoveNode(a));
    assert!(!actor.sizes.contains_key(&a));
    apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.instance(a).unwrap().model.size, size(300.0, 200.0));
}

#[test]
fn pasted_snippets_keep_sizes_and_settings_paste_keeps_destination_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = add(&mut actor);
    apply(&mut actor, ClientMessage::ResizeNode { id, size: size(380.0, 240.0) });
    let snippet = actor.graph_snapshot();
    let events = apply(&mut actor, ClientMessage::Paste(snippet));
    let pasted = events.iter().find_map(|e| match e { ServerMessage::NodeAdded(n) => Some(n), _ => None }).unwrap();
    assert_ne!(pasted.model.id, id);
    assert_eq!(pasted.model.size, size(380.0, 240.0));
    apply(&mut actor, ClientMessage::PasteSettings { id, type_name: "const".into(), properties: vec![("value".into(), DataValue::from("new\nmessage"))] });
    assert_eq!(actor.instance(id).unwrap().model.size, size(380.0, 240.0));
    apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.instance(id).unwrap().model.size, size(380.0, 240.0));
}
