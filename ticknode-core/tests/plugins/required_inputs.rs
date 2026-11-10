// --- Required inputs ---

#[test]
fn make_requires_its_required_fields() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    let def = engine.node_definition(make).unwrap();
    let required: Vec<_> = def
        .pins
        .iter()
        .filter(|p| p.required)
        .map(|p| p.id)
        .collect();
    assert_eq!(
        required,
        [PinId(1), PinId(2)],
        "x and y are required, label is not"
    );

    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine
        .connect_data(make, PinId(0), probe, PinId(1))
        .unwrap();
    engine
        .set_input_value(make, PinId(1), Some(number(1)))
        .unwrap();

    // y is unset: no output value, one error logged (not repeated every tick)
    for _ in 0..3 {
        engine.trigger_node(probe, PinId(0));
        engine.tick();
    }
    let errors: Vec<_> = engine
        .host_state
        .logs
        .iter()
        .filter(|(n, m)| *n == make && m.contains("Required input"))
        .collect();
    assert_eq!(errors.len(), 1, "{:?}", engine.host_state.logs);
    assert!(errors[0].1.contains("\"Y\""));
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);

    // All required fields set: tagged object without the optional field
    engine
        .set_input_value(make, PinId(2), Some(number(2)))
        .unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    let Some((_, Some(DataValue::Object { fields, type_name }))) = trace.inputs.first() else {
        panic!("{:?}", trace.inputs)
    };
    assert_eq!(type_name.as_deref(), Some("test.point"));
    assert_eq!(
        fields.keys().map(String::as_str).collect::<Vec<_>>(),
        ["x", "y"]
    );

    // Optional field appears once provided
    engine
        .set_input_value(make, PinId(3), Some(DataValue::String("a".into())))
        .unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    let Some((_, Some(DataValue::Object { fields, .. }))) = trace.inputs.first() else {
        panic!()
    };
    assert!(fields.contains_key("label"));
}

#[test]
fn connected_source_without_value_skips_silently() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    let source = NodeId::generate();
    engine.create_node(source, "test/counter", &[]).unwrap(); // Count has no value yet
    engine
        .connect_data(source, PinId(1), make, PinId(1))
        .unwrap();
    engine
        .set_input_value(make, PinId(2), Some(number(2)))
        .unwrap();
    let probe = NodeId::generate();
    engine.create_node(probe, "test/debug", &[]).unwrap();
    engine
        .connect_data(make, PinId(0), probe, PinId(1))
        .unwrap();
    engine.trigger_node(probe, PinId(0));
    engine.tick();
    assert!(
        engine.host_state.logs.iter().all(|(n, _)| *n != make),
        "{:?}",
        engine.host_state.logs
    );
    let trace = engine.executed.iter().find(|t| t.node_id == probe).unwrap();
    assert_eq!(trace.inputs, vec![(PinId(1), None)]);
}

#[test]
fn plugin_required_pin_is_respected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    assert!(
        engine
            .node_definition(sink)
            .unwrap()
            .pins
            .iter()
            .any(|p| p.id == PinId(1) && p.required)
    );
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 3), None);
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == sink && m.contains("\"Point\"")),
        "{:?}",
        engine.host_state.logs
    );
}

#[test]
fn foreign_tagged_object_on_named_output_is_rejected() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let bad = NodeId::generate();
    engine.create_node(bad, "test/bad_source", &[]).unwrap();
    engine.trigger_node(bad, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, bad, 1), None);
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == bad && m.contains("other.type")),
        "{:?}",
        engine.host_state.logs
    );

    // A valid output of the declared type passes through with its tag intact
    let good = NodeId::generate();
    engine.create_node(good, "test/point_source", &[]).unwrap();
    engine.trigger_node(good, PinId(0));
    engine.tick();
    assert_eq!(
        output(&engine, good, 1).map(|v| v.get_type()),
        Some(point_type())
    );
}

