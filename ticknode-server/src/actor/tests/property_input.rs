use super::*;

fn add(actor: &mut Actor, type_name: &str, properties: Vec<(String, DataValue)>) -> NodeId {
    test_apply(
        actor,
        ClientMessage::AddNodeWith {
            type_name: type_name.into(),
            position: Position::default(),
            properties,
        },
    )
    .into_iter()
    .find_map(|event| match event {
        ServerMessage::NodeAdded(node) => Some(node.model.id),
        _ => None,
    })
    .unwrap()
}

#[test]
fn ha_target_selection_broadcasts_pin_and_wire_removal_and_undo_restores_both() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/ha_plugin.wasm");
    let Ok(bytes) = std::fs::read(path) else {
        assert!(
            std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(),
            "HA plugin is required"
        );
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    actor.engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let target = add(
        &mut actor,
        "ha/action",
        vec![("action".into(), "lock.set_state".into())],
    );
    let source = add(
        &mut actor,
        "const",
        vec![("value".into(), "lock.front".into())],
    );
    let wire = ConnectionModel {
        src_node: source,
        src_pin: PinId(0),
        dst_node: target,
        dst_pin: PinId(1),
    };
    test_apply(&mut actor, ClientMessage::ConnectData(wire));
    let before = actor.graph_snapshot();
    let events = test_apply(
        &mut actor,
        ClientMessage::SetProperty {
            id: target,
            name: "target".into(),
            value: "lock.back".into(),
        },
    );
    assert!(events.iter().any(
        |e| matches!(e, ServerMessage::DataDisconnected { dst_node, dst_pin }
        if *dst_node == target && *dst_pin == PinId(1))
    ));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::NodeUpdated(node)
        if node.model.id == target && !node.definition.pins.iter().any(|p| p.id == PinId(1))))
    );
    assert!(actor.engine.data_connections().is_empty());
    let events = test_apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::DataConnected(c) if *c == wire))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ServerMessage::NodeUpdated(node)
        if node.model.id == target && node.definition.pins.iter().any(|p| p.id == PinId(1))))
    );
}
