#[test]
fn plugin_typed_properties_validate_without_changing_stored_state() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let rejected = NodeId::generate();
    let error = engine
        .create_node_interactive(
            rejected,
            "test/properties",
            &[("custom".into(), "reject".into())],
        )
        .unwrap_err();
    assert_eq!(error.ui_message().key, "test-plugin-property-rejected");
    assert!(!engine.has_node(rejected));
    let id = NodeId::generate();
    engine.create_node(id, "test/properties", &[]).unwrap();
    let schema = engine.node_definition(id).unwrap().properties;
    assert_eq!(schema[0].kind, PropertyKind::Duration { signed: true });
    assert_eq!(schema[1].kind, PropertyKind::Time);
    assert!(matches!(&schema[2].kind, PropertyKind::MultiSelect(o) if o.len() == 2));
    assert_eq!(schema[3].kind, PropertyKind::Multiline);
    assert_eq!(schema[4].kind, PropertyKind::Hidden);
    let before = engine.export_graph();
    for (name, value, key) in [
        ("duration", "abc", "error-property-duration"),
        ("time", "25:00", "error-property-time"),
        ("days", "wed", "error-property-multi-select"),
        ("custom", "reject", "test-plugin-property-rejected"),
        ("custom", "foreign", ""),
    ] {
        let message = engine
            .set_node_property(id, name, value.into())
            .unwrap_err()
            .ui_message();
        assert_eq!(message.key, key);
        if value == "reject" {
            assert_eq!(message.args, [("name".into(), "custom".into())]);
        }
        assert_eq!(engine.export_graph(), before);
    }
    engine
        .set_node_property(id, "duration", "-01:30:00".into())
        .unwrap();
    engine
        .set_node_property(id, "days", "tue,mon".into())
        .unwrap();
    assert!(
        engine
            .node_model(id)
            .unwrap()
            .properties
            .contains(&("days".into(), "mon,tue".into()))
    );
    let mut graph = engine.export_graph();
    graph.nodes[0]
        .properties
        .iter_mut()
        .find(|(n, _)| n == "custom")
        .unwrap()
        .1 = "reject".into();
    assert!(engine.load_graph(&graph).is_empty());
    assert!(
        engine
            .node_model(id)
            .unwrap()
            .properties
            .contains(&("custom".into(), "reject".into()))
    );
    engine.disable_plugin("test_plugin").unwrap();
    engine
        .enable_plugin("test_plugin", &test_settings("hi", "t"))
        .unwrap();
    assert_eq!(
        engine
            .set_node_property(id, "custom", "reject".into())
            .unwrap_err()
            .ui_message()
            .key,
        "test-plugin-property-rejected"
    );
}
