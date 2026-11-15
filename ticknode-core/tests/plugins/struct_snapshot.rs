// --- Struct snapshot persistence and offline graph restore ---

#[test]
fn struct_snapshot_preserves_pins_and_wires_when_plugin_is_disabled_or_not_loaded() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };

    let source = NodeId::generate();
    engine
        .create_node(
            source,
            "test/point_source",
            &[("x".into(), number(10)), ("y".into(), number(20))],
        )
        .unwrap();

    let brk = NodeId::generate();
    engine
        .create_node(
            brk,
            "object/break",
            &[("type".into(), "test.point".into())],
        )
        .unwrap();

    let make = NodeId::generate();
    engine
        .create_node(
            make,
            "object/make",
            &[("type".into(), "test.point".into())],
        )
        .unwrap();

    let sink = NodeId::generate();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine
        .set_input_value(sink, PinId(2), Some(DataValue::String("manual".into())))
        .unwrap();

    // Wire source -> break (Value)
    engine.connect_data(source, PinId(1), brk, PinId(0)).unwrap();

    // Wire break -> make with field wires:
    // Pin 1: X (Number)
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    // Pin 2: Y (Number)
    engine.connect_data(brk, PinId(2), make, PinId(2)).unwrap();
    // Pin 3: Label (String, optional)
    engine.connect_data(brk, PinId(3), make, PinId(3)).unwrap();

    // Wire make -> sink (Value)
    engine.connect_data(make, PinId(0), sink, PinId(1)).unwrap();

    assert_eq!(engine.data_connections().len(), 5);
    assert!(engine.detached_connections().is_empty());

    // Export graph
    let graph = engine.export_graph();

    // Verify snapshot was exported in node properties
    let make_node = graph.nodes.iter().find(|n| n.id == make).unwrap();
    assert!(make_node.properties.iter().any(|(k, _)| k == "fields"));
    let brk_node = graph.nodes.iter().find(|n| n.id == brk).unwrap();
    assert!(brk_node.properties.iter().any(|(k, _)| k == "fields"));

    // Case 1: Load into a fresh engine where the plugin is NOT loaded at all
    {
        let mut fresh = TickEngine::new();
        let warnings = fresh.load_graph(&graph);
        assert!(warnings.is_empty(), "Warnings on reload: {warnings:?}");
        assert!(fresh.detached_connections().is_empty());
        assert_eq!(fresh.data_connections().len(), 5);

        // Verify pins exist with correct names and types
        let make_def = fresh.node_definition(make).unwrap();
        let brk_def = fresh.node_definition(brk).unwrap();
        assert_eq!(make_def.pins.len(), 4);
        assert_eq!(brk_def.pins.len(), 4);

        assert_eq!(make_def.pins[1].name, "X");
        assert_eq!(make_def.pins[1].pin_type, PinType::Data(DataType::Number));
        assert!(make_def.pins[1].required);
        assert_eq!(make_def.pins[2].name, "Y");
        assert_eq!(make_def.pins[2].pin_type, PinType::Data(DataType::Number));
        assert!(make_def.pins[2].required);
        assert_eq!(make_def.pins[3].name, "Label");
        assert_eq!(make_def.pins[3].pin_type, PinType::Data(DataType::String));
        assert!(!make_def.pins[3].required);

        assert_eq!(brk_def.pins[1].name, "X");
        assert_eq!(brk_def.pins[1].pin_type, PinType::Data(DataType::Number));
        assert_eq!(brk_def.pins[2].name, "Y");
        assert_eq!(brk_def.pins[2].pin_type, PinType::Data(DataType::Number));
        assert_eq!(brk_def.pins[3].name, "Label");
        assert_eq!(brk_def.pins[3].pin_type, PinType::Data(DataType::String));

        // Verify definition type option shows snapshot title "Point"
        let make_type_prop = make_def
            .properties
            .iter()
            .find(|p| p.name == "type")
            .unwrap();
        let PropertyKind::Select(options) = &make_type_prop.kind else {
            panic!("expected Select property");
        };
        let point_opt = options.iter().find(|(v, _)| v == "test.point").unwrap();
        assert_eq!(point_opt.1, "Point");

        // Now load and enable the plugin: wires are kept and Make -> Break works
        let bytes = plugin_bytes("test_plugin").expect("test-plugin.wasm");
        fresh.load_plugin(&bytes, None, |_| vec![]).unwrap();

        assert_eq!(fresh.data_connections().len(), 5);
        assert!(fresh.detached_connections().is_empty());

        fresh.trigger_node(source, PinId(0));
        fresh.trigger_node(sink, PinId(0));
        fresh.tick();

        assert_eq!(
            output(&fresh, sink, 3),
            Some(DataValue::String("manual:10,20".into()))
        );
        assert_eq!(output(&fresh, sink, 4), Some(number(30)));
    }

    // Case 2: Load into a fresh engine where the plugin is added but DISABLED
    {
        let mut disabled_engine = TickEngine::new();
        let bytes = plugin_bytes("test_plugin").expect("test-plugin.wasm");
        disabled_engine
            .add_plugin(&bytes, None, |_| vec![])
            .unwrap();

        let warnings = disabled_engine.load_graph(&graph);
        assert!(warnings.is_empty(), "Warnings on reload: {warnings:?}");
        assert!(disabled_engine.detached_connections().is_empty());
        assert_eq!(disabled_engine.data_connections().len(), 5);

        let make_def = disabled_engine.node_definition(make).unwrap();
        let brk_def = disabled_engine.node_definition(brk).unwrap();
        assert_eq!(make_def.pins.len(), 4);
        assert_eq!(brk_def.pins.len(), 4);
        assert_eq!(make_def.pins[1].name, "X");
        assert_eq!(brk_def.pins[1].name, "X");

        // Enabling the plugin keeps the wires and roundtrip works
        disabled_engine.enable_plugin("test_plugin", &[]).unwrap();

        assert_eq!(disabled_engine.data_connections().len(), 5);
        assert!(disabled_engine.detached_connections().is_empty());

        disabled_engine.trigger_node(source, PinId(0));
        disabled_engine.trigger_node(sink, PinId(0));
        disabled_engine.tick();

        assert_eq!(
            output(&disabled_engine, sink, 3),
            Some(DataValue::String("manual:10,20".into()))
        );
        assert_eq!(output(&disabled_engine, sink, 4), Some(number(30)));
    }

    // Case 3: Plugin was loaded then disabled before graph load
    {
        let mut engine3 = engine_with("test_plugin", vec![]).unwrap();
        engine3.disable_plugin("test_plugin").unwrap();

        let warnings = engine3.load_graph(&graph);
        assert!(warnings.is_empty(), "Warnings on reload: {warnings:?}");
        assert!(engine3.detached_connections().is_empty());
        assert_eq!(engine3.data_connections().len(), 5);

        let make_def = engine3.node_definition(make).unwrap();
        assert_eq!(make_def.pins.len(), 4);

        // Re-enabling keeps wires and roundtrip works
        engine3.enable_plugin("test_plugin", &[]).unwrap();
        assert_eq!(engine3.data_connections().len(), 5);
        assert!(engine3.detached_connections().is_empty());

        engine3.trigger_node(source, PinId(0));
        engine3.trigger_node(sink, PinId(0));
        engine3.tick();

        assert_eq!(
            output(&engine3, sink, 3),
            Some(DataValue::String("manual:10,20".into()))
        );
        assert_eq!(output(&engine3, sink, 4), Some(number(30)));
    }
}
