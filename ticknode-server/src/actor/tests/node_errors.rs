use super::*;

#[test]
fn snapshot_carries_current_node_errors_with_ages_and_roundtrips() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let sheet_id = actor.sheets[0].id;
    let id = NodeId::generate();
    actor.engine.create_node(id, "const", &[]).unwrap();
    actor.node_sheet.insert(id, sheet_id);
    actor.engine.host_state.error(
        id,
        UiMessage::new("error-node-frozen", "Frozen"),
        "Frozen".into(),
    );
    actor.engine.tick();
    let snapshot = actor.snapshot(Some(sheet_id));
    let decoded: ServerMessage = postcard::from_bytes(&encode(&snapshot)).unwrap();
    assert_eq!(snapshot, decoded);
    let ServerMessage::Init { node_errors, .. } = snapshot else {
        panic!()
    };
    assert_eq!(node_errors.len(), 1);
    assert_eq!(node_errors[0].node, id);
    assert_eq!(node_errors[0].count, 1);
    assert_eq!(node_errors[0].message.key, "error-node-frozen");
}
