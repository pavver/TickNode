// --- Property schema and dynamic options ---

fn option_set<'a>(sets: &'a [PropertyOptionSet], source: &str) -> Option<&'a PropertyOptionSet> {
    sets.iter()
        .find(|s| s.plugin == "test_plugin" && s.source == source)
}

#[test]
fn property_schema_and_dynamic_options_follow_the_plugin_lifecycle() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    assert!(
        engine.property_options().is_empty(),
        "nothing is published before the plugin is enabled"
    );
    engine
        .enable_plugin("test_plugin", &test_settings("hi", "t"))
        .unwrap();

    // The schema reaches the definition; the host fills the owning plugin into a dynamic property
    let def = engine
        .registry
        .get_definition("test/picker")
        .unwrap()
        .clone();
    assert_eq!(def.properties.len(), 2);
    assert_eq!(
        def.properties[0].kind,
        PropertyKind::Dynamic {
            source: "greetings".into(),
            plugin: "test_plugin".into()
        }
    );
    assert!(
        matches!(&def.properties[1].kind, PropertyKind::Select(o) if o == &[("a".to_string(), "Option A".to_string()), ("b".to_string(), "Option B".to_string())])
    );
    assert_eq!(def.properties[1].description, "Static options");
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "test/picker",
            &[("choice".into(), DataValue::String("hi".into()))],
        )
        .unwrap();
    assert_eq!(
        engine.node_definition(node).unwrap().properties,
        def.properties
    );

    // Options published from `configure` are stored per (plugin, source)
    let sets = engine.property_options();
    let greetings = option_set(&sets, "greetings").expect("published by configure");
    assert_eq!(
        greetings.options,
        vec![PropertyOption {
            value: "hi".into(),
            label: "HI".into(),
            detail: "detail hi".into(),
            icon: Some("mdi:hand-wave".into())
        }]
    );
    let changes = engine.take_property_option_changes();
    assert_eq!(
        option_set(&changes, "greetings").map(|s| s.options.len()),
        Some(1)
    );
    assert!(
        engine.take_property_option_changes().is_empty(),
        "changes are reported once"
    );

    // Re-configuring with the same value does not mark anything as changed; a new value does
    engine
        .configure_plugin("test_plugin", &test_settings("hi", "t"))
        .unwrap();
    assert!(
        engine.take_property_option_changes().is_empty(),
        "an unchanged list is not re-sent"
    );
    engine
        .configure_plugin("test_plugin", &test_settings("yo", "t"))
        .unwrap();
    let changes = engine.take_property_option_changes();
    assert_eq!(
        option_set(&changes, "greetings").unwrap().options[0].value,
        "yo"
    );

    // Disabling clears the options and reports the sources as empty
    engine.disable_plugin("test_plugin").unwrap();
    assert!(engine.property_options().is_empty());
    let changes = engine.take_property_option_changes();
    assert_eq!(
        option_set(&changes, "greetings").map(|s| s.options.len()),
        Some(0)
    );
    // The stub keeps the schema (read-only in the editor)
    assert_eq!(engine.node_definition(node).unwrap().properties.len(), 2);
}

#[test]
fn option_limits_are_enforced_by_the_host() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    engine
        .enable_plugin("test_plugin", &test_settings("flood", "t"))
        .unwrap();
    let sets = engine.property_options();
    assert_eq!(
        option_set(&sets, "flood").unwrap().options.len(),
        MAX_OPTIONS_PER_SOURCE
    );
    // A value longer than the cap and an empty value are dropped; the valid one stays
    let long = option_set(&sets, "long").unwrap();
    assert_eq!(
        long.options
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["ok"]
    );
}

#[test]
fn ha_plugin_publishes_entity_options_for_the_picker() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let state = |entity: &str, state: &str, attributes: serde_json::Value| {
        serde_json::json!({
            "entity_id": entity, "state": state, "attributes": attributes,
            "last_changed": "2026-10-30T23:15:42.500000+02:00", "last_updated": "2026-10-30T23:15:42.500000+02:00",
            "context": {"id": "01X", "parent_id": null, "user_id": null}
        })
    };
    *ha.entity_list.lock().unwrap() = vec![
        state(
            "light.kitchen",
            "on",
            serde_json::json!({"friendly_name": "Kitchen light"}),
        ),
        state(
            "binary_sensor.door",
            "off",
            serde_json::json!({"friendly_name": "Door", "device_class": "door"}),
        ),
        state("sensor.custom", "1", serde_json::json!({"icon": "mdi:cat"})),
        state(
            "person.alice",
            "home",
            serde_json::json!({"friendly_name":"Alice"}),
        ),
        state(
            "device_tracker.phone",
            "home",
            serde_json::json!({"friendly_name":"Phone"}),
        ),
        state(
            "zone.home",
            "1",
            serde_json::json!({"friendly_name":"Home"}),
        ),
        state(
            "calendar.work",
            "on",
            serde_json::json!({"friendly_name":"Work"}),
        ),
    ];
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    let mut sets = Vec::new();
    for _ in 0..300 {
        engine.tick();
        sets.extend(engine.take_property_option_changes());
        if sets.iter().any(|s| s.source == "entities") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let set = sets
        .iter()
        .find(|s| s.plugin == "home_assistant" && s.source == "entities")
        .expect("the entity options are published once connected");
    let summary: Vec<_> = set
        .options
        .iter()
        .filter(|o| {
            !["person.", "device_tracker.", "zone.", "calendar."]
                .iter()
                .any(|d| o.value.starts_with(d))
        })
        .map(|o| {
            (
                o.value.as_str(),
                o.label.as_str(),
                o.detail.as_str(),
                o.icon.as_deref().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (
                "light.kitchen",
                "Kitchen light",
                "light.kitchen",
                "mdi:lightbulb"
            ),
            (
                "binary_sensor.door",
                "Door",
                "binary_sensor.door",
                "mdi:door-closed"
            ),
            ("sensor.custom", "sensor.custom", "sensor.custom", "mdi:cat"),
        ]
    );
    assert_eq!(set.options.len(), 7);
    for (source, ids) in [
        ("persons", vec!["person.alice", "device_tracker.phone"]),
        ("zones", vec!["zone.home"]),
        ("calendars", vec!["calendar.work"]),
    ] {
        let subset = sets.iter().find(|s| s.source == source).unwrap();
        assert_eq!(
            subset
                .options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ids
        );
        assert!(subset.options.iter().all(|o| set.options.contains(o)));
    }
    let published = engine.property_options();
    assert!(published.iter().any(|s| {
        s.source == "actions:light.kitchen"
            && s.options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>()
                == ["light.turn_on", "light.set_state"]
    }));
    assert!(published.iter().any(|s| {
        s.source == "action_targets"
            && s.options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>()
                == ["light.kitchen"]
    }));
    // Disabling the plugin clears the options
    engine.disable_plugin("home_assistant").unwrap();
    assert!(engine.property_options().is_empty());
    let cleared = engine.take_property_option_changes();
    assert_eq!(cleared.len(), published.len());
    assert!(cleared.iter().all(|s| s.options.is_empty()));
}

#[test]
fn ha_plugin_event_trigger_fires_and_rejections_log_on_the_node() {
    use serde_json::json;
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let ha = FakeHa::start();
    let Some(mut engine) = ha_engine(&ha, HA_TOKEN, &runtime) else {
        return;
    };
    let event_node = NodeId::generate();
    engine
        .create_node(
            event_node,
            "ha/trigger_event",
            &[
                ("event_type".into(), "button_pressed".into()),
                ("event_data".into(), r#"{"button":"door"}"#.into()),
            ],
        )
        .unwrap();
    let event_sink = sink(&mut engine, event_node, 0);
    run_until(&mut engine, |_| {
        ha.trigger_requests.lock().unwrap().len() == 1
    });
    let request = ha.trigger_requests.lock().unwrap()[0].clone();
    assert_eq!(request["type"], "subscribe_trigger");
    assert_eq!(
        request["trigger"],
        json!({"trigger":"event","event_type":["button_pressed"],"event_data":{"button":"door"}})
    );
    ha.fire(
        request["id"].as_u64().unwrap(),
        json!({"platform":"event","event":{"event_type":"button_pressed","data":{"button":"door","count":2}}}),
    );
    count_executions(&mut engine, event_sink, 0, 1);
    assert_eq!(
        output(&engine, event_node, 3),
        Some("button_pressed".into())
    );
    assert_eq!(
        field_text(&output(&engine, event_node, 2).unwrap(), "button").as_deref(),
        Some("door")
    );
    assert_eq!(
        output(&engine, event_node, 1).unwrap().get_type(),
        DataType::Object
    );
    // Local JSON errors cancel the old subscription and retain the user's text for editing.
    engine
        .set_node_property(event_node, "event_data", "{".into())
        .unwrap();
    run_until(&mut engine, |e| {
        e.host_state.logs.iter().any(|(node, message)| {
            *node == event_node && message.contains("Invalid event data JSON")
        })
    });
    run_until(&mut engine, |_| ha.unsubscribed.load(Ordering::SeqCst) >= 1);
    assert_eq!(ha.trigger_requests.lock().unwrap().len(), 1);
    engine
        .set_node_property(event_node, "event_data", "".into())
        .unwrap();
    run_until(&mut engine, |_| {
        ha.trigger_requests.lock().unwrap().len() >= 2
    });
    // A server rejection arrives during plugin-tick, then poll logs it under this node's id.
    engine
        .set_node_property(event_node, "event_type", "reject".into())
        .unwrap();
    run_until(&mut engine, |e| {
        e.host_state.logs.iter().any(|(node, message)| {
            *node == event_node && message.contains("Trigger rejected by fake HA")
        })
    });
}

#[test]
fn ha_plugin_invalid_trigger_json_logs_once_in_node_context_without_a_connection() {
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let mut engine = TickEngine::new();
    engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    engine.enable_plugin("home_assistant", &[]).unwrap();
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "ha/trigger_event",
            &[
                ("event_type".into(), "test".into()),
                ("event_data".into(), "{".into()),
            ],
        )
        .unwrap();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(owner, message)| *owner == node && message.contains("Invalid event data JSON"))
    );
    let errors = engine.node_errors();
    assert!(errors.iter().any(|error| error.node == node
        && error.message.key.is_empty()
        && error.message.fallback.contains("Invalid event data JSON")));
    engine.host_state.logs.clear();
    for _ in 0..3 {
        engine.tick();
        assert!(
            !engine
                .host_state
                .logs
                .iter()
                .any(|(_, message)| message.contains("Invalid event data JSON"))
        );
    }
    engine
        .set_node_property(node, "event_data", "[]".into())
        .unwrap();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(owner, message)| *owner == node
                && message.contains("Event data must be a JSON object"))
    );
    engine.host_state.logs.clear();
    engine
        .set_node_property(node, "event_data", "{}".into())
        .unwrap();
    engine.tick();
    assert!(
        !engine
            .host_state
            .logs
            .iter()
            .any(|(_, message)| message.contains("Event data must"))
    );
    engine.remove_node(node);
}
