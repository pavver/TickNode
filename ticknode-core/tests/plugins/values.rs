#[test]
fn plugin_values_follow_usage_render_and_bind_without_guest_pulls() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    assert_eq!(engine.value_catalogs()[0].namespace, "ha");
    assert_eq!(engine.value_catalogs()[0].entries.len(), 2);
    let template = NodeId::generate();
    engine
        .create_node(
            template,
            "text/template",
            &[("template".into(), "{{ text.ha.sensor.one.state }}".into())],
        )
        .unwrap();
    assert!(
        engine
            .node_definition(template)
            .unwrap()
            .pins
            .iter()
            .all(|p| p.direction == PinDirection::Output)
    );
    let expression = NodeId::generate();
    engine
        .create_node(
            expression,
            "math/expression",
            &[("expression".into(), "ha.sensor.one.state * 2".into())],
        )
        .unwrap();
    let text_sink = NodeId::generate();
    engine.create_node(text_sink, "test/debug", &[]).unwrap();
    engine
        .connect_data(template, PinId(0), text_sink, PinId(1))
        .unwrap();
    engine.trigger_node(text_sink, PinId(0));
    engine.tick();
    assert_eq!(
        engine
            .plugin_values
            .resolve("ha.sensor.one.state")
            .unwrap()
            .0,
        &number(1000)
    );
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, log)| log.contains("Value watch added sensor.one"))
    );
    let first = engine.pure_cache[&template][&PinId(0)].clone();
    assert!(matches!(first, DataValue::String(ref s) if s.contains("kW")));
    engine.trigger_node(text_sink, PinId(0));
    engine.tick();
    assert_ne!(engine.pure_cache[&template][&PinId(0)], first);

    let numeric_sink = NodeId::generate();
    engine.create_node(numeric_sink, "test/debug", &[]).unwrap();
    engine
        .connect_data(expression, PinId(0), numeric_sink, PinId(1))
        .unwrap();
    engine.trigger_node(numeric_sink, PinId(0));
    engine.tick();
    assert_eq!(engine.pure_cache[&expression][&PinId(0)], number(6000));

    // Binding consumes the same watched entry and converts the declared quantity to String.
    let bound = NodeId::generate();
    engine.create_node(bound, "text/to_text", &[]).unwrap();
    engine
        .set_input_variable(bound, PinId(1), VariableRef::parse("ha.sensor.one.state"))
        .unwrap();
    let binding_sink = NodeId::generate();
    engine.create_node(binding_sink, "test/debug", &[]).unwrap();
    engine
        .connect_data(bound, PinId(0), binding_sink, PinId(1))
        .unwrap();
    engine.trigger_node(binding_sink, PinId(0));
    engine.tick();
    assert!(matches!(
        engine.pure_cache[&bound][&PinId(0)],
        DataValue::String(_)
    ));

    engine
        .set_node_property(template, "template", "{{ ha.sensor.two.state }}".into())
        .unwrap();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, log)| log.contains("Value watch added sensor.two"))
    );
    assert!(
        !engine
            .host_state
            .logs
            .iter()
            .any(|(_, log)| log.contains("Value watch removed sensor.one"))
    );
    engine.remove_node(expression);
    engine.set_input_variable(bound, PinId(1), None).unwrap();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, log)| log.contains("Value watch removed sensor.one"))
    );
    engine.remove_node(template);
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, log)| log.contains("Value watch removed sensor.two"))
    );
    engine.disable_plugin("test_plugin").unwrap();
    assert!(engine.value_catalogs().is_empty());
}

#[test]
fn plugin_value_namespace_collision_is_ignored_without_disabling_plugin() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let Some(bytes) = plugin_bytes("ha_plugin") else {
        return;
    };
    let manifest = engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    assert!(manifest.values_namespace.is_none());
    assert_eq!(engine.value_catalogs().len(), 1);
    assert_eq!(engine.value_catalogs()[0].entries.len(), 2);
}

#[test]
fn plugin_value_watches_restore_after_configure_and_instance_restart() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "math/expression",
            &[("expression".into(), "ha.sensor.one.state".into())],
        )
        .unwrap();
    engine.tick();
    engine.host_state.logs.clear();
    engine
        .configure_plugin("test_plugin", &[("token".into(), "ok".into())])
        .unwrap();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, line)| line.contains("Value watch added sensor.one"))
    );
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    engine.host_state.logs.clear();
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(_, line)| line.contains("Value watch added sensor.one"))
    );
    assert!(
        engine
            .plugin_values
            .resolve("ha.sensor.one.state")
            .is_some()
    );
}
