#[test]
fn ha_plugin_entity_changes_expose_numbers_for_match() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    ha.states
        .lock()
        .unwrap()
        .insert("sensor.voltage".into(), "220".into());
    *ha.entity_list.lock().unwrap() = vec![serde_json::json!({
        "entity_id":"sensor.voltage", "state":"220", "attributes":{"unit_of_measurement":"V"},
        "last_changed":"2026-10-30T10:00:00Z", "last_updated":"2026-10-30T10:00:00Z"
    })];
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    run_until(&mut engine, |e| {
        e.property_options().iter().any(|s| s.source == "entities")
    });
    let entity = NodeId::generate();
    engine
        .create_node(
            entity,
            "ha/entity",
            &[("entity_id".into(), "sensor.voltage".into())],
        )
        .unwrap();
    let def = engine.node_definition(entity).unwrap();
    assert_eq!(def.pins[0].pin_type, PinType::Exec);
    assert_eq!(def.pins[1].pin_type, PinType::Data(DataType::Named("quantity.voltage".into())));
    assert_eq!(def.pins[2].pin_type, PinType::Data(DataType::Named("quantity.voltage".into())));
    let changed = sink(&mut engine, entity, 0);
    let decision = NodeId::generate();
    let list = |values: &[&str]| DataValue::List {
        element_type: DataType::String,
        elements: values.iter().map(|s| DataValue::from(*s)).collect(),
    };
    engine
        .create_node(
            decision,
            "match",
            &[
                ("inputs".into(), list(&["1:v:any"])),
                ("cases".into(), list(&["2: v > 230", "3: v < 200"])),
            ],
        )
        .unwrap();
    engine
        .connect_exec(entity, PinId(0), decision, PinId(0))
        .unwrap();
    engine
        .connect_data(entity, PinId(1), decision, PinId(1))
        .unwrap();
    let high = sink(&mut engine, decision, 2);
    let low = sink(&mut engine, decision, 3);
    run_until(&mut engine, |e| {
        output(e, entity, 1) == Some(DataValue::from(220.0))
    });
    assert_eq!(
        output(&engine, entity, 2),
        None,
        "baseline leaves Previous untouched"
    );
    assert!(!engine.executed.iter().any(|t| t.node_id == changed));
    ha.change("sensor.voltage", "220", "240");
    count_executions(&mut engine, high, 0, 1);
    assert_eq!(output(&engine, entity, 1), Some(DataValue::from(240.0)));
    assert_eq!(output(&engine, entity, 2), Some(DataValue::from(220.0)));
    let state = output(&engine, entity, 5).unwrap();
    assert_eq!(state.get_type(), DataType::Named("ha.state".into()));
    assert_eq!(
        field_text(&state, "entity_id").as_deref(),
        Some("sensor.voltage")
    );
    let DataValue::Object { fields, .. } = state else {
        panic!("state")
    };
    assert_eq!(
        fields["last_changed"].value,
        DataValue::DateTime(text::parse_datetime("2026-10-30T21:15:42.500Z").unwrap())
    );
    ha.change("sensor.voltage", "240", "190");
    count_executions(&mut engine, low, 0, 1);
    assert_eq!(output(&engine, entity, 1), Some(DataValue::from(190.0)));
    assert_eq!(output(&engine, entity, 2), Some(DataValue::from(240.0)));
    // All queued updates fire once, with the newest typed values retained.
    ha.change("sensor.voltage", "190", "200");
    ha.change("sensor.voltage", "200", "210");
    count_executions(&mut engine, changed, 0, 2);
    assert_eq!(output(&engine, entity, 1), Some(DataValue::from(210.0)));
    assert_eq!(output(&engine, entity, 2), Some(DataValue::from(200.0)));
}

#[test]
fn ha_plugin_requests_fail_when_the_token_is_wrong_and_nothing_hangs() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, "wrong", &runtime) else {
        return;
    };
    let action = NodeId::generate();
    engine
        .create_node(
            action,
            "ha/action",
            &[("action".into(), "light.turn_on".into())],
        )
        .unwrap();
    let failed = sink(&mut engine, action, u32::MAX - 2);
    // Let the authentication be rejected, then ask: the request fails at once
    run_until(&mut engine, |_| ha.connections.load(Ordering::SeqCst) == 1);
    for _ in 0..30 {
        engine.tick();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, failed, 0, 1);
    let error = output(&engine, action, u32::MAX - 1).unwrap();
    assert!(
        matches!(&error, DataValue::String(e) if !e.is_empty()),
        "{error:?}"
    );
    assert_eq!(
        ha.connections.load(Ordering::SeqCst),
        1,
        "a rejected token is never retried"
    );
}

#[test]
fn ha_plugin_without_a_token_reports_a_settings_error() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let error = engine
        .plugin_settings_error("home_assistant")
        .expect("a missing token is reported");
    assert_eq!(error.key, "home-assistant-error-no-token");
    let error = engine
        .configure_plugin("home_assistant", &ha_settings("http://127.0.0.1:1/", "t"))
        .unwrap_err();
    assert_eq!(error.key, "home-assistant-error-bad-url");
}

#[test]
fn ha_plugin_entity_typed_contract_restores_offline_and_feeds_match() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let entity = NodeId::generate();
    engine
        .create_node(
            entity,
            "ha/entity",
            &[
                ("entity_id".into(), "sensor.voltage".into()),
                ("value_type".into(), "number".into()),
                ("fire_on".into(), "any".into()),
            ],
        )
        .unwrap();
    let list = |items: &[&str]| DataValue::List {
        element_type: DataType::String,
        elements: items.iter().map(|s| DataValue::from(*s)).collect(),
    };
    let decision = NodeId::generate();
    engine
        .create_node(
            decision,
            "match",
            &[
                ("inputs".into(), list(&["1:v:any"])),
                ("cases".into(), list(&["2: v > 230", "3: v < 200"])),
            ],
        )
        .unwrap();
    engine
        .connect_exec(entity, PinId(0), decision, PinId(0))
        .unwrap();
    engine
        .connect_data(entity, PinId(1), decision, PinId(1))
        .unwrap();
    let graph = engine.export_graph();
    let definition = engine.node_definition(entity).unwrap();
    let mut restored = TickEngine::new();
    restored.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(restored.node_definition(entity).unwrap(), definition);
    assert_eq!(definition.pins[1].pin_type, PinType::Data(DataType::Number));
    assert_eq!(definition.pins[2].pin_type, PinType::Data(DataType::Number));
    // Seed cached samples to exercise downstream comparisons without an HA transport.
    for (value, expected) in [(240.0, 2), (190.0, 3), (220.0, 0)] {
        restored
            .data_cache
            .entry(entity)
            .or_default()
            .insert(PinId(1), DataValue::from(value));
        restored.trigger_node(decision, PinId(0));
        restored.tick();
        let trace = restored
            .executed
            .iter()
            .find(|t| t.node_id == decision)
            .unwrap();
        assert_eq!(trace.fired, vec![PinId(expected)]);
    }
}
