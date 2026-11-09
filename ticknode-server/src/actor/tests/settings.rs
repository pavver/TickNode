use super::*;

fn actor(dir: &std::path::Path) -> Actor {
    test_actor(dir)
}

fn apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    test_apply(actor, msg)
}

fn history_state(actor: &Actor) -> HistoryState {
    let sheet_id = actor.sheets[0].id;
    actor.histories.get(&sheet_id).map_or_default(|h| h.state())
}

fn expression(actor: &mut Actor, formula: &str) -> NodeId {
    let id = NodeId::generate();
    let sheet_id = actor.sheets[0].id;
    actor.engine.create_node(id, "math/expression", &[("expression".into(), DataValue::from(formula))]).unwrap();
    actor.positions.insert(id, Position { x: 36.0, y: 80.0 });
    actor.node_sheet.insert(id, sheet_id);
    id
}

fn paste(id: NodeId, formula: &str) -> ClientMessage {
    ClientMessage::PasteSettings {
        id, type_name: "math/expression".into(),
        properties: vec![("expression".into(), DataValue::from(formula)), ("in:1".into(), DataValue::from(7.0))],
    }
}

#[test]
fn settings_paste_preserves_id_position_valid_wires_and_undo_restores_removed_wire() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let source = expression(&mut actor, "42");
    let id = expression(&mut actor, "a + b + c");
    actor.engine.set_input_value(id, PinId(1), Some(DataValue::from(3.0))).unwrap();
    actor.engine.set_input_value(id, PinId(2), Some(DataValue::from(9.0))).unwrap();
    let valid = ConnectionModel { src_node: source, src_pin: PinId(0), dst_node: id, dst_pin: PinId(2) };
    let removed = ConnectionModel { dst_pin: PinId(3), ..valid };
    for c in [valid, removed] { actor.engine.connect_data(c.src_node, c.src_pin, c.dst_node, c.dst_pin).unwrap(); }
    let before = actor.graph_snapshot();
    let msg = paste(id, "a + b");
    assert_eq!(postcard::from_bytes::<ClientMessage>(&postcard::to_allocvec(&msg).unwrap()).unwrap(), msg);
    let events = apply(&mut actor, msg);
    let model = actor.engine.node_model(id).unwrap();
    assert_eq!(model.id, id);
    assert_eq!(model.type_name, "math/expression");
    assert_eq!(model.properties, vec![("expression".into(), DataValue::from("a + b")), ("in:1".into(), DataValue::from(7.0))]);
    assert_eq!(actor.positions[&id], Position { x: 36.0, y: 80.0 });
    assert_eq!(actor.engine.data_connections(), &[valid]);
    assert!(events.iter().any(|e| matches!(e, ServerMessage::DataDisconnected { dst_node, dst_pin } if *dst_node == id && *dst_pin == removed.dst_pin)));
    assert!(events.iter().any(|e| matches!(e, ServerMessage::NodeUpdated(n) if n.model.id == id && n.model.position == actor.positions[&id])));
    assert!(!events.contains(&ServerMessage::NodeRemoved(id)), "recreating the same ID must preserve editor selection");
    assert!(!events.contains(&ServerMessage::DataDisconnected { dst_node: id, dst_pin: valid.dst_pin }), "valid wire selection must survive");
    assert!(!events.contains(&ServerMessage::DataConnected(valid)), "unchanged wires need no event");
    assert!(history_state(&actor).can_undo);
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(events.contains(&ServerMessage::DataConnected(removed)));
    assert!(!history_state(&actor).can_undo, "paste must add exactly one history entry");
    apply(&mut actor, ClientMessage::Redo);
    assert_eq!(actor.engine.data_connections(), &[valid]);
}

#[test]
fn settings_paste_rejects_type_mismatch_unknown_nodes_and_stubs_without_changing_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet_id = actor.sheets[0].id;
    let id = expression(&mut actor, "a");
    apply(&mut actor, paste(id, "b"));
    apply(&mut actor, ClientMessage::Undo);
    let stub = NodeId::generate();
    let mut graph = actor.graph_snapshot();
    graph.nodes.push(NodeModel { id: stub, type_name: "missing/node".into(), properties: vec![], position: Position::default(), pins: None, size: None });
    actor.engine.load_graph(&graph);
    actor.node_sheet.insert(stub, sheet_id);
    let before = actor.graph_snapshot();
    let history = history_state(&actor);
    actor.dirty_since = None;
    let (reply, _) = mpsc::unbounded_channel();
    let error = actor.apply(ClientMessage::PasteSettings { id, type_name: "const".into(), properties: vec![] }, Some(sheet_id), &reply).unwrap_err();
    assert_eq!(error.key, "error-settings-type-mismatch");
    assert_eq!(error.args, vec![("from".into(), "const".into()), ("to".into(), "math/expression".into())]);
    let missing = NodeId::generate();
    assert_eq!(actor.apply(paste(missing, "1"), Some(sheet_id), &reply).unwrap_err(), EngineError::NodeNotFound(missing).ui_message());
    let reason = actor.engine.node_unavailable(stub).unwrap();
    assert!(!reason.key.is_empty());
    assert_eq!(actor.apply(ClientMessage::PasteSettings { id: stub, type_name: "missing/node".into(), properties: vec![] }, Some(sheet_id), &reply).unwrap_err(), EngineError::PluginUnavailable(reason).ui_message());
    assert_eq!(actor.graph_snapshot(), before);
    assert_eq!(history_state(&actor), history);
    assert!(actor.dirty_since.is_none());
}

#[test]
fn settings_paste_identical_properties_has_no_events_history_or_dirty_mark() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = expression(&mut actor, "a");
    let properties = actor.engine.node_model(id).unwrap().properties;
    let before = actor.graph_snapshot();
    let events = apply(&mut actor, ClientMessage::PasteSettings { id, type_name: "math/expression".into(), properties });
    assert!(events.is_empty());
    assert_eq!(actor.graph_snapshot(), before);
    assert_eq!(history_state(&actor), HistoryState::default());
    assert!(actor.dirty_since.is_none());
}

#[test]
fn settings_paste_is_allowed_in_batch_and_replaces_split_properties() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let sheet_id = actor.sheets[0].id;
    actor.engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
    ])], &[]).unwrap();
    let id = NodeId::generate();
    actor.engine.create_node(id, "type/break/test.point", &[]).unwrap();
    actor.node_sheet.insert(id, sheet_id);
    let before = actor.graph_snapshot();
    let properties = vec![("split:0".into(), DataValue::Bool(true))];
    apply(&mut actor, ClientMessage::Batch(vec![
        ClientMessage::PasteSettings { id, type_name: "type/break/test.point".into(), properties: properties.clone() },
        ClientMessage::MoveNode { id, position: Position { x: 12.0, y: 24.0 } },
    ]));
    assert_eq!(actor.engine.node_model(id).unwrap().properties, properties);
    assert!(actor.engine.node_definition(id).unwrap().pins.iter().any(|p| p.id == split_field_pin(PinId(0), 0).unwrap()));
    apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(!history_state(&actor).can_undo);
}
