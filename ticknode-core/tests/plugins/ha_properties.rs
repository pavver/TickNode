#[test]
fn ha_typed_properties_refresh_definitions_and_report_legacy_errors_without_tcp() {
    let Some(mut engine) = engine_with("ha_plugin", vec![]) else {
        return;
    };
    let node = NodeId::generate();
    engine.create_node(node, "ha/condition_sun", &[]).unwrap();
    let property_kind = |engine: &TickEngine, name: &str| {
        engine
            .node_definition(node)
            .unwrap()
            .properties
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
            .kind
    };
    assert_eq!(property_kind(&engine, "after_offset"), PropertyKind::Hidden);
    engine
        .set_node_property(node, "after", "sunset".into())
        .unwrap();
    assert_eq!(
        property_kind(&engine, "after_offset"),
        PropertyKind::Duration { signed: true }
    );
    engine
        .set_node_property(node, "after_offset", "-30m".into())
        .unwrap();
    engine
        .set_node_property(node, "after", "none".into())
        .unwrap();
    assert_eq!(property_kind(&engine, "after_offset"), PropertyKind::Hidden);
    assert!(
        engine
            .node_model(node)
            .unwrap()
            .properties
            .contains(&("after_offset".into(), "-30m".into()))
    );
    for (kind, properties, key) in [
        (
            "ha/condition_time",
            vec![("after".into(), "25:00".into())],
            "home-assistant-error-time",
        ),
        (
            "ha/condition_sun",
            vec![
                ("before".into(), "sunrise".into()),
                ("before_offset".into(), "bad".into()),
            ],
            "home-assistant-error-signed-duration",
        ),
        (
            "ha/trigger_mqtt",
            vec![("topic".into(), "a/#/b".into())],
            "home-assistant-error-mqtt-topic",
        ),
        (
            "ha/trigger_time_pattern",
            vec![("hours".into(), "/0".into())],
            "home-assistant-error-time-pattern",
        ),
    ] {
        engine.remove_node(node);
        engine.create_node(node, kind, &properties).unwrap();
        engine.host_state.logs.clear();
        engine.tick();
        assert!(
            engine
                .node_errors()
                .iter()
                .any(|e| e.node == node && e.message.key == key),
            "{kind}"
        );
        assert_eq!(
            engine
                .host_state
                .logs
                .iter()
                .filter(|(owner, _)| *owner == node)
                .count(),
            1
        );
        engine.host_state.logs.clear();
        for _ in 0..3 {
            engine.trigger_node(node, PinId(0));
            engine.tick();
            assert!(
                engine
                    .node_errors()
                    .iter()
                    .any(|e| e.node == node && e.message.key == key)
            );
        }
        assert!(
            !engine
                .host_state
                .logs
                .iter()
                .any(|(owner, _)| *owner == node)
        );
    }
    engine
        .set_node_property(node, "hours", "12".into())
        .unwrap();
    engine.tick();
    assert!(!engine.node_errors().iter().any(|e| e.node == node));
}

#[test]
fn ha_action_property_input_modes_remove_wires_and_preserve_legacy_wires_as_detached() {
    let Some(mut engine) = engine_with("ha_plugin", vec![]) else {
        return;
    };
    let action = NodeId::generate();
    engine
        .create_node(
            action,
            "ha/action",
            &[("action".into(), "lock.set_state".into())],
        )
        .unwrap();
    let def = engine.node_definition(action).unwrap();
    assert_eq!(def.properties[0].input, Some(PinId(1)));
    assert!(def.pins.iter().any(|p| p.id == PinId(1)));
    assert!(
        def.pins
            .iter()
            .any(|p| p.id == PinId(2) && p.required && p.pin_type == PinType::Data(DataType::Bool))
    );
    constant(&mut engine, "lock.front".into(), action, 1);
    let mut legacy = engine.export_graph();
    legacy
        .nodes
        .iter_mut()
        .find(|n| n.id == action)
        .unwrap()
        .properties
        .iter_mut()
        .find(|(n, _)| n == "target")
        .unwrap()
        .1 = "lock.back".into();
    assert!(engine.load_graph(&legacy).is_empty());
    assert!(engine.data_connections().is_empty());
    assert_eq!(engine.detached_connections().len(), 1);
    assert_eq!(
        engine.detached_connections()[0].connection,
        legacy.data_connections[0]
    );
    assert!(
        !engine
            .node_definition(action)
            .unwrap()
            .pins
            .iter()
            .any(|p| p.id == PinId(1))
    );
    engine
        .set_node_property(action, "target", "".into())
        .unwrap();
    constant(&mut engine, "lock.front".into(), action, 1);
    let removed = engine
        .set_node_property(action, "target", "lock.back".into())
        .unwrap();
    assert_eq!(removed.data.len(), 1);
    assert!(engine.data_connections().is_empty());
    let def = engine.node_definition(action).unwrap();
    assert!(
        matches!(&def.properties[1].kind, PropertyKind::Dynamic { source, .. } if source == "actions:lock.back")
    );
    assert!(
        def.pins
            .iter()
            .any(|p| p.id == PinId(2) && p.required && p.pin_type == PinType::Data(DataType::Bool))
    );
}
