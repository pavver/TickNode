use super::*;

#[test]
fn persistence_graph_and_snippets() {
    let mut engine = setup_engine();

    let make = NodeId::generate();
    let brk = NodeId::generate();
    engine
        .create_node(make, "object/make", &[("type".into(), "ha.state".into())])
        .unwrap();
    engine
        .create_node(brk, "object/break", &[("type".into(), "ha.state".into())])
        .unwrap();
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();

    // 1. Export and reload graph
    let graph = engine.export_graph();
    let mut reloaded = setup_engine();
    let warnings = reloaded.load_graph(&graph);
    assert!(warnings.is_empty(), "Warnings on reload: {warnings:?}");

    let make_def = reloaded.fresh_node_definition(make).unwrap();
    let brk_def = reloaded.fresh_node_definition(brk).unwrap();
    assert_eq!(make_def.pins.len(), 6);
    assert_eq!(brk_def.pins.len(), 6);
    assert_eq!(reloaded.connections.data().len(), 1);

    // 2. Export snippet and import snippet
    let snippet = engine.export_snippet(&[make, brk]);
    let mut snippet_engine = setup_engine();
    let (imported_nodes, warnings) = snippet_engine.import_snippet(&snippet);
    assert!(
        warnings.is_empty(),
        "Warnings on snippet import: {warnings:?}"
    );
    assert_eq!(imported_nodes.len(), 2);
    for (_, new_id) in imported_nodes {
        let def = snippet_engine.fresh_node_definition(new_id).unwrap();
        assert_eq!(def.pins.len(), 6);
    }
}

#[test]
fn plugin_type_removal_and_readdition_with_appended_fields() {
    let mut engine = TickEngine::new();

    // Register telegram types owned by "telegram"
    let t_types = telegram_types();
    engine
        .register_types_owned(Some("telegram"), &t_types, &[])
        .unwrap();

    let make = NodeId::generate();
    let brk = NodeId::generate();
    engine
        .create_node(
            make,
            "object/make",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .create_node(
            brk,
            "object/break",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();

    let make_def_before = engine.fresh_node_definition(make).unwrap();
    let brk_def_before = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(make_def_before.pins.len(), 7); // 0: out, 1..=6: fields
    assert_eq!(brk_def_before.pins.len(), 7); // 0: in, 1..=6: fields
    assert_eq!(engine.connections.data().len(), 1);

    // Disable plugin / remove owned types
    // Simulate what lifecycle.rs does on disable:
    let type_names: Vec<String> = engine
        .type_owners
        .iter()
        .filter(|(_, o)| *o == "telegram")
        .map(|(t, _)| t.clone())
        .collect();
    for t in type_names {
        engine.type_owners.remove(&t);
        engine.types.remove(&t);
    }
    engine.registry.unregister_owner("telegram");
    engine.notify_types_changed();

    // Node keeps stored `type` and its last field snapshot:
    // Pins and connections survive!
    let make_def_mid = engine.fresh_node_definition(make).unwrap();
    let brk_def_mid = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(make_def_mid.pins.len(), 7);
    assert_eq!(brk_def_mid.pins.len(), 7);
    assert_eq!(engine.connections.data().len(), 1);
    assert!(!engine.frozen.contains_key(&make));
    assert!(!engine.frozen.contains_key(&brk));

    // Re-register telegram types with an appended field "bio" on telegram.user
    let mut updated_types = telegram_types();
    for t in &mut updated_types {
        if t.name == "telegram.user" {
            let mut fields = t.fields().to_vec();
            fields.push(FieldDefinition::new("bio", "Bio", DataType::String).optional());
            *t = TypeDefinition::new_struct("telegram.user", "Telegram user", fields);
        }
    }

    engine
        .register_types_owned(Some("telegram"), &updated_types, &[])
        .unwrap();

    // The new pins appear!
    let make_def_after = engine.fresh_node_definition(make).unwrap();
    let brk_def_after = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(make_def_after.pins.len(), 8); // 1 extra field pin
    assert_eq!(brk_def_after.pins.len(), 8);
    assert_eq!(make_def_after.pins[7].name, "Bio");
    assert_eq!(brk_def_after.pins[7].name, "Bio");

    // Existing connection survived and nodes are not frozen because the addition was non-breaking!
    assert_eq!(engine.connections.data().len(), 1);
    assert!(!engine.frozen.contains_key(&make));
    assert!(!engine.frozen.contains_key(&brk));
}

#[test]
fn property_change_prunes_invalid_connections() {
    let mut engine = setup_engine();

    let make = NodeId::generate();
    let brk = NodeId::generate();
    engine
        .create_node(
            make,
            "object/make",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine
        .create_node(
            brk,
            "object/break",
            &[("type".into(), "telegram.user".into())],
        )
        .unwrap();
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();

    // Connect a field output
    let sink = NodeId::generate();
    engine.create_node(sink, "test/debug", &[]).unwrap();
    engine.connect_data(brk, PinId(6), sink, PinId(1)).unwrap(); // is_bot (Bool) to sink

    assert_eq!(engine.connections.data().len(), 2);

    // Change type on make from telegram.user to ha.state:
    // make Pin 0 is now Named("ha.state"), while brk Pin 0 expects Named("telegram.user")
    let removed = engine
        .set_node_property(make, "type", "ha.state".into())
        .unwrap();
    assert_eq!(removed.data.len(), 1); // connection between make and brk pruned!

    // brk still has telegram.user pins:
    assert_eq!(engine.fresh_node_definition(brk).unwrap().pins.len(), 7);
    // connection from brk to sink survives:
    assert_eq!(engine.connections.data().len(), 1);
}
