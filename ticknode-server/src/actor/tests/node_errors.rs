use super::*;

#[test]
fn snapshot_carries_current_node_errors_with_ages_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "const", &[]).unwrap();
    engine.host_state.error(id, UiMessage::new("error-node-frozen", "Frozen"), "Frozen".into());
    engine.tick();
    let (events, _) = broadcast::channel(16);
    let actor = Actor {
        engine, positions: HashMap::new(), sizes: HashMap::new(), routes: HashMap::new(), storage: Storage::new(dir.path().join("graph.json")),
        plugins: Plugins::new(dir.path()), events, dirty_since: None, traces: HashMap::new(), history: Default::default(), telemetry: super::super::telemetry::Telemetry::new(Instant::now()), frames: Default::default(),
    };
    let snapshot = actor.snapshot();
    let decoded: ServerMessage = postcard::from_bytes(&encode(&snapshot)).unwrap();
    assert_eq!(snapshot, decoded);
    let ServerMessage::Init { node_errors, .. } = snapshot else { panic!() };
    assert_eq!(node_errors.len(), 1);
    assert_eq!(node_errors[0].node, id);
    assert_eq!(node_errors[0].count, 1);
    assert_eq!(node_errors[0].message.key, "error-node-frozen");
}
