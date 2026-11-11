#[test]
fn ha_plugin_entity_emits_canonical_watts_and_action_sends_kilowatts() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    *ha.entity_list.lock().unwrap() = vec![serde_json::json!({
        "entity_id":"sensor.power", "state":"2", "attributes":{
            "unit_of_measurement":"kW", "device_class":"power"
        },
        "last_changed":"2026-10-30T10:00:00Z", "last_updated":"2026-10-30T10:00:00Z"
    })];
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    run_until(&mut engine, |e| {
        e.property_options().iter().any(|s| s.source == "entities")
            && e.property_options()
                .iter()
                .any(|s| s.options.iter().any(|o| o.value == "number.set_value"))
    });
    let entity = NodeId::generate();
    engine
        .create_node(
            entity,
            "ha/entity",
            &[("entity_id".into(), "sensor.power".into())],
        )
        .unwrap();
    let definition = engine.node_definition(entity).unwrap();
    for pin in [1, 2] {
        assert_eq!(
            definition.pins[pin].pin_type,
            PinType::Data(DataType::Named("quantity.power".into()))
        );
    }
    run_until(&mut engine, |e| output(e, entity, 1) == Some(2000.0.into()));
    assert_eq!(output(&engine, entity, 3), Some("kW".into()));
    ha.change("sensor.power", "2", "3");
    run_until(&mut engine, |e| output(e, entity, 1) == Some(3000.0.into()));
    assert_eq!(output(&engine, entity, 2), Some(2000.0.into()));
    assert_eq!(
        field_text(&output(&engine, entity, 5).unwrap(), "state"),
        Some("3".into())
    );

    let action = NodeId::generate();
    engine
        .create_node(
            action,
            "ha/action",
            &[("action".into(), "number.set_value".into())],
        )
        .unwrap();
    assert_eq!(
        engine.node_definition(action).unwrap().pins[2].pin_type,
        PinType::Data(DataType::Named("quantity.power".into()))
    );
    engine
        .connect_data(entity, PinId(1), action, PinId(2))
        .unwrap();
    engine.trigger_node(action, PinId(0));
    run_until(&mut engine, |_| !ha.calls.lock().unwrap().is_empty());
    assert_eq!(
        ha.calls.lock().unwrap()[0]["service_data"],
        serde_json::json!({"value":3.0})
    );

    // The selector unit survives a new component instance with no service catalog.
    let graph = engine.export_graph();
    let action_definition = engine.node_definition(action).unwrap();
    let mut offline = TickEngine::new();
    offline
        .load_plugin(&plugin_bytes("ha_plugin").unwrap(), None, |_| vec![])
        .unwrap();
    assert!(offline.load_graph(&graph).is_empty());
    assert_eq!(offline.node_definition(action).unwrap(), action_definition);
    assert_eq!(offline.export_graph(), graph);
}

#[test]
fn ha_quantity_contracts_and_selector_units_restore_without_a_connection() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let list = |values: &[&str]| DataValue::List {
        element_type: DataType::String,
        elements: values.iter().map(|s| DataValue::from(*s)).collect(),
    };
    let entity = NodeId::generate();
    engine
        .create_node(
            entity,
            "ha/entity",
            &[
                ("entity_id".into(), "sensor.power".into()),
                ("value_type".into(), "quantity.power".into()),
            ],
        )
        .unwrap();
    let action = NodeId::generate();
    engine
        .create_node(
            action,
            "ha/action",
            &[
                ("action".into(), "number.set_value".into()),
                ("fields".into(), list(&["value:quantity.power"])),
                ("field_labels".into(), list(&["Power"])),
                ("required_fields".into(), list(&["value"])),
                ("field_units".into(), list(&["kW"])),
            ],
        )
        .unwrap();
    engine
        .connect_data(entity, PinId(1), action, PinId(2))
        .unwrap();
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(
        restored.node_definition(entity),
        engine.node_definition(entity)
    );
    assert_eq!(
        restored.node_definition(action),
        engine.node_definition(action)
    );
    let definition = restored.node_definition(action).unwrap();
    assert_eq!(
        definition.pins[2].pin_type,
        PinType::Data(DataType::Named("quantity.power".into()))
    );
}
