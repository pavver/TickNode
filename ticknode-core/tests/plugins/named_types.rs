// --- Custom data types ---

fn point_type() -> DataType {
    DataType::Named("test.point".into())
}

fn mode_type() -> DataType {
    DataType::Named("test.mode".into())
}

#[test]
fn plugin_types_are_registered_with_make_and_break_nodes() {
    let Some(engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let point = engine
        .types
        .get("test.point")
        .expect("the type is registered");
    assert!(point.is_struct());
    assert_eq!(point.color.as_deref(), Some("#e0a030"));
    assert_eq!(engine.types.get("test.mode").unwrap().values().len(), 2);

    let make = engine
        .registry
        .get_definition("type/make/test.point")
        .unwrap();
    assert_eq!(make.title, "Make Point");
    assert_eq!(make.category, "Data");
    assert!(!make.help.is_empty());
    assert!(make.pins.iter().any(|p| p.id == PinId(0)
        && p.direction == PinDirection::Output
        && p.pin_type == PinType::Data(point_type())));
    assert_eq!(
        make.pins
            .iter()
            .filter(|p| p.direction == PinDirection::Input)
            .count(),
        3
    );
    let brk = engine
        .registry
        .get_definition("type/break/test.point")
        .unwrap();
    assert!(!brk.help.is_empty());
    assert!(
        brk.pins
            .iter()
            .any(|p| p.id == PinId(0) && p.direction == PinDirection::Input)
    );
    // Make/Break nodes are not generated for enums
    assert!(
        engine
            .registry
            .get_definition("type/make/test.mode")
            .is_none()
    );
}

#[test]
fn type_conflicts_and_unknown_types_are_rejected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let before = engine.registry.get_definitions().len();

    let dup = TypeDefinition::new_enum("test.mode", "Other", &[("a", "A")]);
    let err = engine.register_types(&[dup], &[]).unwrap_err();
    assert!(err.contains("test.mode"), "{err}");

    let unknown = TypeDefinition::new_struct(
        "other.thing",
        "Thing",
        vec![FieldDefinition::new(
            "p",
            "P",
            DataType::Named("nope.type".into()),
        )],
    );
    let err = engine.register_types(&[unknown], &[]).unwrap_err();
    assert!(err.contains("nope.type"), "{err}");

    let bad_name = TypeDefinition::new_enum("nodot", "X", &[("a", "A")]);
    assert!(engine.register_types(&[bad_name], &[]).is_err());

    // Node pin referencing an unknown type
    let node = NodeDefinition {
        adaptive: false,
        cost: 1,
        properties: Vec::new(),
        type_name: "other/node".into(),
        title: "N".into(),
        category: "Test".into(),
        description: String::new(),
        help: String::new(),
        pins: vec![PinDefinition {
            id: PinId(0),
            name: "P".into(),
            pin_type: PinType::Data(DataType::Named("nope.type".into())),
            direction: PinDirection::Input,
            required: false,
        }],
    };
    let err = engine.register_types(&[], &[node]).unwrap_err();
    assert!(err.contains("nope.type"), "{err}");

    // A field may reference a previously registered type
    let nested = TypeDefinition::new_struct(
        "other.nested",
        "Nested",
        vec![FieldDefinition::new("p", "P", point_type())],
    );
    engine.register_types(&[nested], &[]).unwrap();
    assert!(
        engine
            .registry
            .get_definition("type/make/other.nested")
            .is_some()
    );
    assert_eq!(engine.registry.get_definitions().len(), before + 2);

    // Failed attempt left no partial state behind
    assert!(engine.types.get("other.thing").is_none());
}

#[test]
fn pin_compatibility_respects_named_types() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    let brk = NodeId::generate();
    engine
        .create_node(brk, "type/break/test.point", &[])
        .unwrap();
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    let get_field = NodeId::generate();
    engine
        .create_node(get_field, "object/get_field", &[])
        .unwrap();
    let object = NodeId::generate();
    engine.create_node(object, "object/create", &[]).unwrap();
    let debug = NodeId::generate();
    engine.create_node(debug, "test/debug", &[]).unwrap();

    // Named -> Named (same), Named -> Any, Named -> Object
    engine.connect_data(make, PinId(0), brk, PinId(0)).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine
        .connect_data(make, PinId(0), debug, PinId(1))
        .unwrap();
    engine
        .connect_data(make, PinId(0), get_field, PinId(0))
        .unwrap();
    // Object -> Named is rejected (strict typing)
    assert!(matches!(
        engine.connect_data(object, PinId(0), brk, PinId(0)),
        Err(EngineError::TypeMismatch(..))
    ));
    assert!(matches!(
        engine.connect_data(object, PinId(0), sink, PinId(1)),
        Err(EngineError::TypeMismatch(..))
    ));
    // Different Named type and String -> Enum
    assert!(engine.connect_data(make, PinId(0), sink, PinId(2)).is_err());
    let text = NodeId::generate();
    engine
        .create_node(
            text,
            "const",
            &[("value".into(), DataValue::String("auto".into()))],
        )
        .unwrap();
    engine.connect_data(text, PinId(0), sink, PinId(2)).unwrap();
    // Number -> Named
    let num = NodeId::generate();
    engine
        .create_node(num, "const", &[("value".into(), number(1))])
        .unwrap();
    assert!(engine.connect_data(num, PinId(0), sink, PinId(1)).is_err());
}

#[test]
fn enum_pin_to_string_is_allowed() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    // Break has no enum output, so test the rule directly through the type registry
    assert!(engine.types.compatible(&mode_type(), &DataType::String));
    assert!(!engine.types.compatible(&DataType::String, &mode_type()));
    assert!(engine.types.compatible(&point_type(), &DataType::Object));
    assert!(!engine.types.compatible(&DataType::Object, &point_type()));
    assert!(engine.types.compatible(
        &DataType::List(Box::new(point_type())),
        &DataType::List(Box::new(DataType::Any))
    ));
    assert!(!engine.types.compatible(&mode_type(), &point_type()));
    let _ = &mut engine;
}

#[test]
fn make_and_break_roundtrip_through_plugin_nodes() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let source = NodeId::generate();
    engine
        .create_node(
            source,
            "test/point_source",
            &[("x".into(), number(5)), ("y".into(), number(7))],
        )
        .unwrap();
    let brk = NodeId::generate();
    engine
        .create_node(brk, "type/break/test.point", &[])
        .unwrap();
    engine
        .connect_data(source, PinId(1), brk, PinId(0))
        .unwrap();

    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    // y is an inline value on the node; label is optional but provided
    engine
        .set_input_value(make, PinId(2), Some(number(1)))
        .unwrap();
    engine
        .set_input_value(make, PinId(3), Some(DataValue::String("hi".into())))
        .unwrap();

    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine
        .set_input_value(sink, PinId(2), Some(DataValue::String("manual".into())))
        .unwrap();

    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(
        output(&engine, sink, 3),
        Some(DataValue::String("manual:5,1:hi".into()))
    );
    assert_eq!(output(&engine, sink, 4), Some(number(6)));

    // Value is tagged
    let made = engine
        .pure_cache
        .get(&make)
        .and_then(|m| m.get(&PinId(0)))
        .cloned()
        .or_else(|| {
            engine
                .executed
                .iter()
                .find(|t| t.node_id == make)
                .and_then(|t| t.outputs.first().map(|(_, v)| v.clone()))
        });
    assert_eq!(made.map(|v| v.get_type()), Some(point_type()));
}

#[test]
fn break_skips_missing_optional_field() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let source = NodeId::generate();
    engine
        .create_node(source, "test/point_source", &[])
        .unwrap();
    let brk = NodeId::generate();
    engine
        .create_node(brk, "type/break/test.point", &[])
        .unwrap();
    engine
        .connect_data(source, PinId(1), brk, PinId(0))
        .unwrap();
    engine.trigger_node(source, PinId(0));
    engine.tick();

    // Break evaluates on demand; the point has no label
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine.connect_data(brk, PinId(3), probe, PinId(1)).unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == probe)
        .expect("debug has run");
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);
    let trace = engine
        .executed
        .iter()
        .find(|t| t.node_id == brk)
        .expect("break was evaluated");
    assert!(trace.outputs.iter().any(|(p, _)| *p == PinId(1)));
    assert!(!trace.outputs.iter().any(|(p, _)| *p == PinId(3)));
}

#[test]
fn enum_input_values_are_validated() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine
        .set_input_value(sink, PinId(2), Some(DataValue::String("auto".into())))
        .unwrap();
    assert!(matches!(
        engine.set_input_value(sink, PinId(2), Some(DataValue::String("bogus".into()))),
        Err(EngineError::TypeMismatch(..))
    ));
    assert!(
        engine
            .set_input_value(sink, PinId(2), Some(number(1)))
            .is_err()
    );
    // A struct cannot be set inline on a node
    assert!(
        engine
            .set_input_value(sink, PinId(1), Some(DataValue::String("x".into())))
            .is_err()
    );
    // Value survived rejected attempts
    let model = engine.node_model(sink).unwrap();
    assert!(
        model
            .properties
            .contains(&("in:2".to_string(), DataValue::String("auto".into())))
    );
}

#[test]
fn graph_with_named_nodes_survives_save_and_load() {
    let Some(bytes) = plugin_bytes("test_plugin") else {
        return;
    };
    let new_engine = || {
        let mut engine = TickEngine::new();
        engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
        engine
    };
    let mut engine = new_engine();
    let source = NodeId::generate();
    engine
        .create_node(
            source,
            "test/point_source",
            &[("x".into(), number(2)), ("y".into(), number(3))],
        )
        .unwrap();
    let brk = NodeId::generate();
    engine
        .create_node(brk, "type/break/test.point", &[])
        .unwrap();
    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine
        .connect_data(source, PinId(1), brk, PinId(0))
        .unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    engine.connect_data(brk, PinId(2), make, PinId(2)).unwrap();
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();
    engine
        .set_input_value(sink, PinId(2), Some(DataValue::String("auto".into())))
        .unwrap();

    // Roundtrip through JSON, as stored on disk
    let json = serde_json::to_string_pretty(&engine.export_graph()).unwrap();
    let graph: GraphModel = serde_json::from_str(&json).unwrap();
    let mut restored = new_engine();
    let warnings = restored.load_graph(&graph);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(restored.export_graph(), engine.export_graph());

    restored.trigger_node(source, PinId(0));
    restored.trigger_node(sink, PinId(0));
    restored.tick();
    assert_eq!(
        output(&restored, sink, 3),
        Some(DataValue::String("auto:2,3".into()))
    );

    // Without the plugin, the typed node is not lost: a stub preserves its type, properties, and connections
    let mut bare = TickEngine::new();
    assert!(bare.load_graph(&graph).is_empty());
    assert_eq!(bare.export_graph(), graph);
    assert!(bare.node_unavailable(source).is_some());
}

