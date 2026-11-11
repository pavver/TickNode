// Opaque IDs retain their declared contracts and use the owning plugin's live labels.
fn chat_type() -> DataType {
    DataType::Named("opaque.chat".into())
}
const CHAT_ID: i64 = 9_007_199_254_740_993;

#[test]
fn identifier_plugins_validate_outputs_inline_values_and_connection_contracts() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let chat = engine.types.get("opaque.chat").unwrap();
    assert!(chat.is_identifier());
    assert_eq!(chat.plugin.as_deref(), Some("test_plugin"));
    assert!(engine.registry.get_definition("type/make/opaque.chat").is_none());
    let source = NodeId::generate();
    let target = NodeId::generate();
    engine.create_node(source, "test/identifier", &[]).unwrap();
    engine.create_node(target, "test/identifier", &[]).unwrap();
    engine.connect_data(source, PinId(2), target, PinId(1)).unwrap();
    assert!(engine.connect_data(source, PinId(3), target, PinId(1)).is_err());
    let number = NodeId::generate();
    engine.create_node(number, "const", &[("value".into(), 1i64.into())]).unwrap();
    assert!(engine.connect_data(number, PinId(0), target, PinId(1)).is_err());
    for value in [DataValue::Number(NumberValue::Float(1.0)), "123".into()] {
        assert!(engine.set_input_value(target, PinId(1), Some(value)).is_err());
    }
    engine.set_input_value(source, PinId(1), Some((-100_123i64).into())).unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, source, 2), Some((-100_123i64).into()));
    engine.set_input_value(source, PinId(1), None).unwrap();
    for invalid in [DataValue::Number(NumberValue::Float(1.0)), "123".into(), true.into()] {
        engine.set_node_property(source, "value", invalid).unwrap();
        engine.data_cache.remove(&source);
        engine.trigger_node(source, PinId(0));
        engine.tick();
        assert_eq!(output(&engine, source, 2), None);
        assert!(engine.host_state.logs.iter().any(|(_, s)| s.contains("non-integer")));
    }
}

#[test]
fn identifier_labels_reach_to_text_templates_variables_and_update_after_configure() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let source = NodeId::generate();
    engine.create_node(source, "test/identifier", &[]).unwrap();
    let text = NodeId::generate();
    engine.create_node(text, "text/to_text", &[]).unwrap();
    engine.connect_data(source, PinId(2), text, PinId(1)).unwrap();
    assert_eq!(engine.node_definition(text).unwrap().pins[0].pin_type, PinType::Data(chat_type()));
    let template = NodeId::generate();
    engine
        .create_node(template, "text/template", &[("template".into(), "{{ text.chat }} / {{ chat }}".into())])
        .unwrap();
    let pin = engine.node_definition(template).unwrap().pins.into_iter().find(|p| p.name == "chat").unwrap().id;
    engine.connect_data(source, PinId(2), template, pin).unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();
    let pull = |engine: &mut TickEngine, node| {
        engine.trigger_node(node, PinId(0));
        engine.tick();
        output(engine, node, 0)
    };
    assert_eq!(pull(&mut engine, text), Some("Hello chat".into()));
    assert_eq!(pull(&mut engine, template), Some(format!("Hello chat / {CHAT_ID}").into()));
    engine
        .configure_plugin("test_plugin", &[("token".into(), "good".into()), ("greeting".into(), "Renamed".into())])
        .unwrap();
    assert_eq!(pull(&mut engine, text), Some("Renamed chat".into()));
    engine.set_node_property(source, "value", (-100i64).into()).unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();
    assert_eq!(pull(&mut engine, text), Some("-100".into()));
    let variable = VariableKey::global("chat");
    assert!(engine.variables.create(variable.clone(), chat_type(), CHAT_ID.into()));
    let get = NodeId::generate();
    engine
        .create_node(
            get,
            "variable/get",
            &[("name".into(), "chat".into()), ("value_type".into(), "opaque.chat".into())],
        )
        .unwrap();
    assert_eq!(pull(&mut engine, get), Some(CHAT_ID.into()));
    engine.set_node_property(get, "value_type", "number".into()).unwrap();
    assert_eq!(pull(&mut engine, get), None, "a typed variable cannot launder an ID into Number");
    engine.set_node_property(get, "value_type", "opaque.user".into()).unwrap();
    assert_eq!(pull(&mut engine, get), None);
    engine.disable_plugin("test_plugin").unwrap();
    assert!(engine.types.get("opaque.chat").is_none());
    assert!(engine.property_options().is_empty());
    engine.enable_plugin("test_plugin", &[("token".into(), "good".into())]).unwrap();
    assert_eq!(engine.variables.get(&variable).unwrap().value, CHAT_ID.into());
}

#[test]
fn identifier_variable_writes_preserve_the_connected_source_contract() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let source = NodeId::generate();
    engine.create_node(source, "test/identifier", &[]).unwrap();
    let key = VariableKey::global("chat");
    engine.variables.create(key.clone(), chat_type(), 0i64.into());
    let set = NodeId::generate();
    engine.create_node(set, "variable/set", &[("name".into(), "chat".into())]).unwrap();
    engine.connect_data(source, PinId(2), set, PinId(3)).unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();
    engine.trigger_node(set, PinId(0));
    engine.tick();
    assert_eq!(engine.variables.get(&key).unwrap().value, CHAT_ID.into());
    assert!(engine.executed.iter().any(|t| t.node_id == set && t.fired.contains(&PinId(1))));
    // Both opaque types have the same integer representation; the foreign contract must still fail.
    engine.connect_data(source, PinId(3), set, PinId(3)).unwrap();
    engine.trigger_node(set, PinId(0));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == set && t.fired.contains(&PinId(2))));
    let ordinary = VariableKey::global("number");
    engine.variables.create(ordinary.clone(), DataType::Number, 0i64.into());
    engine.set_node_property(set, "name", "number".into()).unwrap();
    engine.connect_data(source, PinId(2), set, PinId(3)).unwrap();
    engine.trigger_node(set, PinId(0));
    engine.tick();
    assert_eq!(engine.variables.get(&ordinary).unwrap().value, 0i64.into());
    assert!(engine.executed.iter().any(|t| t.node_id == set && t.fired.contains(&PinId(2))));
}

#[test]
fn identifier_variable_create_defaults_to_an_integer_and_survives_graph_restore() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else { return };
    let create = NodeId::generate();
    engine
        .create_node(
            create,
            "variable/create",
            &[
                ("name".into(), "destination".into()),
                ("scope".into(), "global".into()),
                ("value_type".into(), "opaque.chat".into()),
            ],
        )
        .unwrap();
    let graph = engine.export_graph();
    engine.load_graph(&graph);
    engine.trigger_node(create, PinId(0));
    engine.tick();
    let variable = engine.variables.get(&VariableKey::global("destination")).unwrap();
    assert_eq!(variable.data_type, chat_type());
    assert_eq!(variable.value, 0i64.into());
}
