use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, properties: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

fn pull(engine: &mut TickEngine, id: NodeId, pin: u32) -> Option<DataValue> {
    engine.pull_data(id, PinId(pin), &mut Vec::new())
}

fn property(name: &str, value: &str) -> (String, DataValue) {
    (name.into(), DataValue::from(value))
}

#[test]
fn logical_nodes_use_inline_values_and_default_to_false() {
    for (name, default) in [("logical/and", false), ("logical/or", false), ("logical/not", true)] {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, name, &[]);
        assert_eq!(pull(&mut engine, id, 2), Some(DataValue::Bool(default)));
        engine.set_input_value(id, PinId(0), Some(DataValue::Bool(true))).unwrap();
        assert_eq!(pull(&mut engine, id, 2), Some(DataValue::Bool(name == "logical/or")));
        if name != "logical/not" {
            engine.set_input_value(id, PinId(1), Some(DataValue::Bool(true))).unwrap();
            assert_eq!(pull(&mut engine, id, 2), Some(DataValue::Bool(true)));
        }
    }
}

#[test]
fn expressions_evaluate_variables_and_preserve_zero_fallback() {
    for (expression, expected) in [("2 + 3", 5.0), ("2.5", 2.5), ("true", 0.0), ("(", 0.0), ("a * 2 + b", 6.0)] {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, "math/expression", &[property("expression", expression)]);
        if expression.contains('a') {
            engine.set_input_value(id, PinId(1), Some(DataValue::from(3i64))).unwrap();
        }
        assert_eq!(pull(&mut engine, id, 0), Some(DataValue::from(expected)), "{expression}");
        let model = engine.node_model(id).unwrap();
        assert!(model.properties.contains(&property("expression", expression)));
    }
}

#[test]
fn collection_type_properties_share_parsing_and_fallback_rules() {
    for (name, key) in [
        ("list/create", "element_type"), ("list/add", "element_type"), ("list/get", "element_type"),
        ("list/remove", "element_type"), ("list/length", "element_type"),
        ("object/set_field", "field_type"), ("object/get_field", "field_type"),
    ] {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, name, &[property(key, "string")]);
        assert!(engine.node_model(id).unwrap().properties.contains(&property(key, "string")));
        engine.set_node_property(id, "unrelated", DataValue::from("bool")).unwrap();
        engine.set_node_property(id, key, DataValue::Bool(true)).unwrap();
        assert!(engine.node_model(id).unwrap().properties.contains(&property(key, "string")));
        engine.set_node_property(id, key, DataValue::from("invalid")).unwrap();
        assert!(engine.node_model(id).unwrap().properties.contains(&property(key, "number")));
    }
}

#[test]
fn list_index_nodes_preserve_missing_negative_fractional_and_out_of_bounds_behavior() {
    for (index, expected_item, remaining) in [
        (None, None, vec![10i64, 20i64]),
        (Some(-1.0), None, vec![10i64, 20i64]),
        (Some(2.0), None, vec![10i64, 20i64]),
        (Some(0.9), Some(10i64), vec![20i64]),
        (Some(1.0), Some(20i64), vec![10i64]),
    ] {
        let mut engine = TickEngine::new();
        let get = add(&mut engine, "list/get", &[]);
        let remove = add(&mut engine, "list/remove", &[]);
        let length = add(&mut engine, "list/length", &[]);
        assert_eq!(pull(&mut engine, get, 2), None);
        assert_eq!(pull(&mut engine, remove, 2), None);
        let source = add(&mut engine, "const", &[("value".into(), DataValue::List {
            element_type: DataType::Number, elements: vec![DataValue::from(10i64), DataValue::from(20i64)],
        })]);
        for id in [get, remove, length] {
            engine.connect_data(source, PinId(0), id, PinId(0)).unwrap();
        }
        for id in [get, remove] {
            engine.set_input_value(id, PinId(1), index.map(DataValue::from)).unwrap();
        }
        assert_eq!(pull(&mut engine, get, 2), expected_item.map(DataValue::from));
        assert_eq!(pull(&mut engine, remove, 2), Some(DataValue::List {
            element_type: DataType::Number, elements: remaining.into_iter().map(DataValue::from).collect(),
        }));
        assert_eq!(pull(&mut engine, length, 1), Some(DataValue::from(2i64)));
        assert_eq!(pull(&mut engine, source, 0), Some(DataValue::List {
            element_type: DataType::Number, elements: vec![DataValue::from(10i64), DataValue::from(20i64)],
        }));
    }
}

#[test]
fn object_get_reports_found_and_leaves_missing_value_empty() {
    let mut engine = TickEngine::new();
    let get = add(&mut engine, "object/get_field", &[property("field_type", "number")]);
    assert_eq!(pull(&mut engine, get, 3), Some(DataValue::Bool(false)));
    let source = add(&mut engine, "const", &[("value".into(), DataValue::Object {
        fields: [("answer".into(), ObjectField { name: "answer".into(), description: String::new(), value: DataValue::from(42i64) })].into(),
        type_name: None,
    })]);
    engine.connect_data(source, PinId(0), get, PinId(0)).unwrap();
    for (name, found, value) in [("answer", true, Some(DataValue::from(42i64))), ("missing", false, None)] {
        engine.set_input_value(get, PinId(1), Some(DataValue::from(name))).unwrap();
        assert_eq!(pull(&mut engine, get, 3), Some(DataValue::Bool(found)));
        assert_eq!(pull(&mut engine, get, 2), value);
    }
}

#[test]
fn variable_properties_restore_the_key_used_by_both_nodes() {
    let mut engine = TickEngine::new();
    engine.variables.create(VariableKey::global("answer"), DataType::Number, 0i64.into());
    let get = add(&mut engine, "variable/get", &[property("var_name", "answer")]);
    let set = add(&mut engine, "variable/set", &[property("var_name", "answer")]);
    engine.set_input_value(set, PinId(3), Some(DataValue::from(42i64))).unwrap();
    engine.trigger_node(set, PinId(0));
    engine.tick();
    assert_eq!(pull(&mut engine, get, 0), Some(DataValue::from(42i64)));
    for id in [get, set] {
        engine.set_node_property(id, "other", DataValue::from("ignored")).unwrap();
        engine.set_node_property(id, "var_name", DataValue::Bool(false)).unwrap();
        assert!(engine.node_model(id).unwrap().properties.contains(&property("name", "answer")));
    }
}

#[test]
fn console_formats_scalars_and_nested_collections_for_the_user() {
    for (value, expected) in [
        (DataValue::Bool(true), "true"),
        (DataValue::from(42i64), "42"),
        (DataValue::from(2.5), "2.5"),
        (DataValue::from("text"), "text"),
        (DataValue::Time(ticknode_shared::text::parse_time("07:30").unwrap()), "07:30:00"),
        (DataValue::DateTime(ticknode_shared::text::parse_datetime("2026-01-01").unwrap()), "2026-01-01 00:00:00 UTC"),
        (DataValue::Duration(std::time::Duration::from_millis(1500)), "1.5s"),
        (DataValue::Color(Color { r: 1, g: 2, b: 3, a: 4 }), "Color(r:1, g:2, b:3, a:4)"),
        (DataValue::List { element_type: DataType::Number, elements: vec![DataValue::from(42i64)] }, "[42]"),
        (DataValue::Object {
            fields: [("nested".into(), ObjectField { name: "nested".into(), description: String::new(), value: DataValue::List { element_type: DataType::String, elements: vec![DataValue::from("value")] } })].into(),
            type_name: None,
        }, "{nested: [value]}"),
    ] {
        let mut engine = TickEngine::new();
        let source = add(&mut engine, "const", &[("value".into(), value)]);
        let console = add(&mut engine, "console", &[]);
        engine.connect_data(source, PinId(0), console, PinId(1)).unwrap();
        engine.trigger_node(console, PinId(0));
        engine.tick();
        assert_eq!(engine.host_state.logs, vec![(console, expected.to_string())]);
    }
}

#[test]
fn engine_errors_keep_translation_keys_and_arguments() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    for (error, key, args) in [
        (EngineError::CycleDetected(a, b), "error-cycle-detected", vec![("node".into(), format!("{a:?}")), ("dependency".into(), format!("{b:?}"))]),
        (EngineError::TypeMismatch(DataType::Number, DataType::Bool), "error-type-mismatch", vec![("src".into(), "number".into()), ("dst".into(), "bool".into())]),
        (EngineError::InvalidPinConnection, "error-invalid-pin-connection", vec![]),
        (EngineError::NodeNotFound(a), "error-node-not-found", vec![("node".into(), format!("{a:?}"))]),
        (EngineError::UnknownNodeType("missing".into()), "error-unknown-node-type", vec![("type".into(), "missing".into())]),
    ] {
        let message = error.ui_message();
        assert_eq!(message.key, key);
        assert_eq!(message.args, args);
        assert_eq!(message.fallback, error.to_string());
    }
    let reason = UiMessage::new("plugin-error", "Unavailable").arg("detail", "disabled");
    assert_eq!(EngineError::PluginUnavailable(reason.clone()).ui_message(), reason);
}
