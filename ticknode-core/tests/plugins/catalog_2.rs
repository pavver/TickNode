#[test]
fn ha_plugin_action_form_sends_fields_target_and_response_requests() {
    use serde_json::json;
    const DONE: u32 = u32::MAX - 3;
    const FAILED: u32 = u32::MAX - 2;
    const ERROR: u32 = u32::MAX - 1;
    const RESPONSE: u32 = u32::MAX;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    let action = NodeId::generate();
    engine.create_node(action, "ha/action", &[]).unwrap();
    engine
        .set_node_property(action, "target", "light.kitchen".into())
        .unwrap();
    run_until(&mut engine, |e| {
        e.property_options()
            .iter()
            .any(|s| s.source == "actions:light.kitchen")
    });
    let actions = engine
        .property_options()
        .into_iter()
        .find(|s| s.source == "actions:light.kitchen")
        .unwrap();
    let option = &actions.options[0];
    assert_eq!(
        (
            option.label.as_str(),
            option.detail.as_str(),
            option.icon.as_deref()
        ),
        ("Turn on", "light.turn_on", Some("mdi:lightbulb"))
    );
    engine
        .set_node_property(action, "action", "light.turn_on".into())
        .unwrap();
    let def = engine.node_definition(action).unwrap();
    assert!(def.pins.iter().any(|p| p.id == PinId(2)
        && p.name == "Brightness"
        && p.required
        && p.pin_type == PinType::Data(DataType::Number)));
    assert_eq!(
        def.pins
            .iter()
            .filter(|p| p.direction == PinDirection::Input)
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        [
            "In",
            "Brightness",
            "effect",
            "rgb_color",
            "transition"
        ]
    );
    assert_eq!(
        def.properties[0].kind,
        PropertyKind::Dynamic {
            source: "action_targets".into(),
            plugin: "home_assistant".into()
        }
    );
    assert_eq!(
        def.properties[1].kind,
        PropertyKind::Dynamic {
            source: "actions:light.kitchen".into(),
            plugin: "home_assistant".into()
        }
    );
    let done = sink(&mut engine, action, DONE);
    let failed = sink(&mut engine, action, FAILED);
    // Required fields are enforced before any request can reach HA.
    engine.trigger_node(action, PinId(0));
    engine.tick();
    assert!(ha.calls.lock().unwrap().is_empty());
    engine
        .set_input_value(action, PinId(2), Some(number(120)))
        .unwrap();
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, done, 0, 1);
    let calls = ha.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["service_data"], json!({"brightness":120}));
    assert_eq!(calls[0]["target"], json!({"entity_id":["light.kitchen"]}));
    assert!(calls[0].get("return_response").is_none());
    assert_eq!(output(&engine, action, ERROR), Some("".into()));
    assert!(output(&engine, action, RESPONSE).is_none());
    // Empty target exposes Entity and preserves the snapshotted action fields.
    engine.set_node_property(action, "target", "".into()).unwrap();
    let def = engine.node_definition(action).unwrap();
    assert!(def.pins.iter().any(|p| p.id == PinId(1)
        && p.direction == PinDirection::Input && p.name == "Entity"));
    assert!(def.pins.iter().any(|p| p.id == PinId(2) && p.name == "Brightness"));
    // Both an inline value and a connected input work in input mode (the connection wins).
    engine
        .set_input_value(action, PinId(1), Some("light.inline".into()))
        .unwrap();
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, done, 0, 1);
    assert_eq!(
        ha.calls.lock().unwrap()[1]["target"],
        json!({"entity_id":["light.inline"]})
    );
    constant(&mut engine, "light.connected".into(), action, 1);
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, done, 0, 1);
    assert_eq!(
        ha.calls.lock().unwrap()[2]["target"],
        json!({"entity_id":["light.connected"]})
    );
    // A response-capable service requests and exposes only its response object.
    engine
        .set_node_property(action, "action", "weather.get_forecasts".into())
        .unwrap();
    engine
        .set_input_value(action, PinId(2), Some("daily".into()))
        .unwrap();
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, done, 0, 1);
    let calls = ha.calls.lock().unwrap().clone();
    assert_eq!(calls[3]["return_response"], true);
    assert_eq!(calls[3]["service_data"], json!({"type":"daily"}));
    let DataValue::Object { fields, .. } = output(&engine, action, RESPONSE).unwrap() else {
        panic!("response object")
    };
    assert!(fields.contains_key("weather.home"));
    engine
        .set_node_property(action, "action", "broken.missing".into())
        .unwrap();
    engine.trigger_node(action, PinId(0));
    count_executions(&mut engine, failed, 0, 1);
    assert!(
        matches!(output(&engine,action,ERROR),Some(DataValue::String(e)) if e.contains("Service not found"))
    );

    // Saved field IDs, labels, required flags and inputs survive a fresh, offline plugin instance.
    engine
        .set_node_property(action, "action", "light.turn_on".into())
        .unwrap();
    engine
        .set_input_value(action, PinId(2), Some(number(80)))
        .unwrap();
    let definition = engine.node_definition(action).unwrap();
    let graph = engine.export_snippet(&[action]);
    let bytes = plugin_bytes("ha_plugin").unwrap();
    let mut offline = TickEngine::new();
    offline.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(offline.load_graph(&graph).is_empty());
    assert_eq!(offline.node_definition(action).unwrap(), definition);
    assert_eq!(
        offline.export_graph().nodes[0].properties,
        graph.nodes[0].properties
    );
    offline.trigger_node(action, PinId(0));
    offline.tick();
    assert!(
        matches!(output(&offline,action,ERROR),Some(DataValue::String(e)) if e.contains("not configured"))
    );
}

#[test]
fn ha_plugin_entity_emissions_remain_pullable_across_ticks() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    ha.states
        .lock()
        .unwrap()
        .insert("sensor.temperature".into(), "19.5".into());
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    let entity = NodeId::generate();
    engine
        .create_node(
            entity,
            "ha/entity",
            &[
                ("entity_id".into(), "sensor.temperature".into()),
                ("value_type".into(), "number".into()),
            ],
        )
        .unwrap();
    assert!(
        engine
            .node_definition(entity)
            .unwrap()
            .pins
            .iter()
            .all(|p| p.direction == PinDirection::Output)
    );
    run_until(&mut engine, |e| {
        output(e, entity, 1) == Some(DataValue::from(19.5))
    });
    ha.change("sensor.temperature", "19.5", "21.5");
    run_until(&mut engine, |e| {
        output(e, entity, 1) == Some(DataValue::from(21.5))
    });
    for _ in 0..8 {
        engine.tick();
    }
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine
        .connect_data(entity, PinId(1), probe, PinId(1))
        .unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert!(
        trace
            .inputs
            .contains(&(PinId(1), Some(DataValue::from(21.5)))),
        "{trace:?}"
    );
    // Retarget an unknown entity offline: keep the type until manually overridden.
    engine
        .set_node_property(entity, "entity_id", "light.kitchen".into())
        .unwrap();
    engine
        .set_node_property(entity, "value_type", "bool".into())
        .unwrap();
    run_until(&mut engine, |e| {
        output(e, entity, 1) == Some(DataValue::Bool(false))
    });
    ha.change("light.kitchen", "off", "on");
    run_until(&mut engine, |e| {
        output(e, entity, 1) == Some(DataValue::Bool(true))
    });
    for _ in 0..8 {
        engine.tick();
    }
    engine
        .connect_data(entity, PinId(1), probe, PinId(1))
        .unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert!(
        trace
            .inputs
            .contains(&(PinId(1), Some(DataValue::Bool(true)))),
        "{trace:?}"
    );
    ha.change("light.kitchen", "on", "unavailable");
    run_until(&mut engine, |e| {
        output(e, entity, 5)
            .is_some_and(|v| field_text(&v, "state").as_deref() == Some("unavailable"))
    });
    assert_eq!(output(&engine, entity, 1), None);
    assert_eq!(output(&engine, entity, 2), Some(DataValue::Bool(true)));
}

#[test]
fn ha_plugin_action_snapshot_restores_offline_through_wasm() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let action = NodeId::generate();
    let list = |values: &[&str]| DataValue::List {
        element_type: DataType::String,
        elements: values.iter().map(|s| DataValue::from(*s)).collect(),
    };
    engine
        .create_node(
            action,
            "ha/action",
            &[
                ("action".into(), "light.turn_on".into()),
                ("target".into(), "light.kitchen".into()),
                (
                    "fields".into(),
                    list(&["brightness:number", "rgb_color:color"]),
                ),
                ("field_labels".into(), list(&["Brightness", "RGB color"])),
                ("required_fields".into(), list(&["brightness"])),
                ("target_domains".into(), list(&["light"])),
                ("supports_response".into(), DataValue::Bool(false)),
                ("in:2".into(), number(80)),
            ],
        )
        .unwrap();
    let def = engine.node_definition(action).unwrap();
    assert_eq!(def.pins[1].id, PinId(2));
    assert_eq!(def.pins[1].name, "Brightness");
    assert!(def.pins[1].required);
    assert_eq!(def.pins[2].pin_type, PinType::Data(DataType::Color));
    assert_eq!(
        def.properties[0].kind,
        PropertyKind::Dynamic {
            source: "action_targets".into(),
            plugin: "home_assistant".into()
        }
    );
    assert_eq!(
        def.properties[1].kind,
        PropertyKind::Dynamic {
            source: "actions:light.kitchen".into(),
            plugin: "home_assistant".into()
        }
    );
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.node_definition(action).unwrap(), def);
    assert_eq!(restored.export_graph(), graph);
    restored.trigger_node(action, PinId(0));
    restored.tick();
    assert!(
        matches!(output(&restored, action, u32::MAX - 1), Some(DataValue::String(e)) if e.contains("not configured"))
    );
    assert!(
        restored
            .executed
            .iter()
            .any(|t| t.node_id == action && t.fired.contains(&PinId(u32::MAX - 2)))
    );
}

#[test]
fn ha_plugin_template_condition_completes_asynchronously_and_unsubscribes() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    let gate = NodeId::generate();
    engine
        .create_node(
            gate,
            "ha/condition_template",
            &[("value_template".into(), "{{ true }}".into())],
        )
        .unwrap();
    let yes = sink(&mut engine, gate, 1);
    let no = sink(&mut engine, gate, 2);
    run_until(&mut engine, |_| {
        ha.subs.lock().unwrap().contains_key("__cache")
    });
    let before = ha.unsubscribed.load(Ordering::SeqCst);
    engine.trigger_node(gate, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, gate, 3), None, "rendering is asynchronous");
    count_executions(&mut engine, yes, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(true)));
    run_until(&mut engine, |_| {
        ha.unsubscribed.load(Ordering::SeqCst) > before
    });
    engine
        .set_node_property(gate, "value_template", "{{ false }}".into())
        .unwrap();
    engine.trigger_node(gate, PinId(0));
    count_executions(&mut engine, no, 0, 1);
    assert_eq!(output(&engine, gate, 3), Some(DataValue::Bool(false)));
}

#[test]
fn plugin_clears_data_and_rejects_clears_of_other_pins() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let node = NodeId::generate();
    engine.create_node(node, "test/clear", &[]).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, node, 1), Some(number(42)));
    engine.trigger_node(node, PinId(2));
    engine.tick();
    assert_eq!(output(&engine, node, 1), None);
    assert!(
        engine
            .executed
            .iter()
            .find(|t| t.node_id == node)
            .unwrap()
            .outputs
            .is_empty()
    );
    let errors: Vec<_> = engine
        .host_state
        .logs
        .iter()
        .filter(|(id, text)| *id == node && text.contains("not a data output"))
        .collect();
    assert_eq!(
        errors.len(),
        4,
        "input, exec output and unknown pins must be rejected: {errors:?}"
    );
}
