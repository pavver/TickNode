// --- Disabling and enabling a plugin ---

#[test]
fn disabling_plugin_keeps_nodes_as_stubs_and_enabling_restores_them() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    engine
        .enable_plugin("test_plugin", &test_settings("Hi", "good"))
        .unwrap();

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
    let sink_node = NodeId::generate();
    engine
        .create_node(sink_node, "test/point_sink", &[])
        .unwrap();
    let make = NodeId::generate();
    engine
        .create_node(make, "type/make/test.point", &[])
        .unwrap();
    let debug = NodeId::generate();
    engine.create_node(debug, "test/debug", &[]).unwrap();
    engine
        .connect_data(source, PinId(1), brk, PinId(0))
        .unwrap();
    engine.connect_data(brk, PinId(1), make, PinId(1)).unwrap();
    engine.connect_data(brk, PinId(2), make, PinId(2)).unwrap();
    engine
        .connect_data(make, PinId(0), sink_node, PinId(1))
        .unwrap();
    engine
        .set_input_value(
            sink_node,
            PinId(2),
            Some(DataValue::String("manual".into())),
        )
        .unwrap();
    engine
        .connect_data(sink_node, PinId(3), debug, PinId(1))
        .unwrap();
    let before = engine.export_graph();
    assert!(
        before
            .nodes
            .iter()
            .find(|n| n.id == sink_node)
            .unwrap()
            .properties
            .iter()
            .any(|(k, _)| k == "in:2")
    );

    let change = engine.disable_plugin("test_plugin").unwrap();
    assert_eq!(
        change.replaced.len(),
        4,
        "source, brk, sink, make: {:?}",
        change.replaced
    );
    assert!(change.removed.data.is_empty());
    // Types unregistered from registries
    assert!(
        engine
            .registry
            .get_definition("test/point_source")
            .is_none()
    );
    assert!(
        engine
            .registry
            .get_definition("type/make/test.point")
            .is_none()
    );
    assert!(engine.types.get("test.point").is_none());
    assert!(!engine.plugin_active("test_plugin"));
    // Graph is identical: types, properties (including in:*), and connections
    assert_eq!(engine.export_graph(), before);
    // Stub: provides reason, rejects changes, does not execute
    assert_eq!(
        engine
            .node_unavailable(source)
            .map(|e| e.fallback)
            .as_deref(),
        Some("plugin test_plugin unavailable")
    );
    assert!(engine.node_unavailable(debug).is_none());
    assert!(matches!(
        engine.set_node_property(source, "x", number(9)),
        Err(EngineError::PluginUnavailable(_))
    ));
    assert!(matches!(
        engine.set_input_value(sink_node, PinId(2), None),
        Err(EngineError::PluginUnavailable(_))
    ));
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == source && m.contains("plugin test_plugin unavailable"))
    );
    // Definition is no longer in the palette, but the node still retains its pins
    assert_eq!(engine.node_definition(source).unwrap().pins.len(), 2);

    // Disabling again is an error; a graph saved while disabled is also preserved intact
    assert!(engine.disable_plugin("test_plugin").is_err());
    let json = serde_json::to_string(&engine.export_graph()).unwrap();
    let mut reloaded = TickEngine::new();
    assert!(
        reloaded
            .load_graph(&serde_json::from_str(&json).unwrap())
            .is_empty()
    );
    assert_eq!(reloaded.export_graph(), before);

    // Enabling: stubs become real nodes again, connections remain intact
    let change = engine
        .enable_plugin("test_plugin", &test_settings("Hi", "good"))
        .unwrap();
    assert_eq!(change.replaced.len(), 4);
    assert!(change.removed.data.is_empty() && change.removed.exec.is_empty());
    assert!(engine.node_unavailable(source).is_none());
    assert_eq!(engine.export_graph(), before);
    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink_node, PinId(0));
    engine.tick();
    assert_eq!(
        output(&engine, sink_node, 3),
        Some(DataValue::String("manual:2,3".into()))
    );

    // A graph reloaded with stubs also revives once the plugin becomes available
    let bytes = plugin_bytes("test_plugin").unwrap();
    reloaded.add_plugin(&bytes, None, |_| vec![]).unwrap();
    let change = reloaded
        .enable_plugin("test_plugin", &test_settings("Hi", "good"))
        .unwrap();
    assert_eq!(change.replaced.len(), 4);
    assert_eq!(reloaded.export_graph(), before);
    reloaded.trigger_node(source, PinId(0));
    reloaded.trigger_node(sink_node, PinId(0));
    reloaded.tick();
    assert_eq!(
        output(&reloaded, sink_node, 3),
        Some(DataValue::String("manual:2,3".into()))
    );
}

#[test]
fn enabling_detaches_connections_that_became_invalid() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    let counter = NodeId::generate();
    let debug = NodeId::generate();
    let ghost_link = ConnectionModel {
        src_node: counter,
        src_pin: PinId(9),
        dst_node: debug,
        dst_pin: PinId(0),
    };
    let graph = GraphModel {
        nodes: vec![
            NodeModel {
                id: counter,
                type_name: "test/counter".into(),
                properties: vec![("step".into(), number(2))],
                position: Position::default(),
                pins: None,
                size: None,
            },
            NodeModel {
                id: debug,
                type_name: "test/debug".into(),
                properties: vec![("prefix".into(), DataValue::String(String::new()))],
                position: Position::default(),
                pins: None,
                size: None,
            },
        ],
        // Pin 9 was inferred by the stub from the connection, but the real counter node has no such pin
        exec_connections: vec![ghost_link],
        data_connections: vec![ConnectionModel {
            src_node: counter,
            src_pin: PinId(1),
            dst_node: debug,
            dst_pin: PinId(1),
        }],
        wire_routes: Vec::new(),
        detached_connections: Vec::new(),
        frames: Vec::new(),
    };
    let mut graph = graph;
    graph.nodes.sort_by_key(|n| n.id.0);
    assert!(engine.load_graph(&graph).is_empty());
    let graph = engine.export_graph();
    assert!(graph.nodes.iter().all(|n| n.pins.is_some()));

    let change = engine
        .enable_plugin("test_plugin", &test_settings("Hi", "good"))
        .unwrap();
    assert_eq!(change.replaced, vec![counter]);
    assert!(change.removed.exec.is_empty());
    assert_eq!(change.detached.len(), 1);
    assert_eq!(change.detached[0].connection, ghost_link);
    assert_eq!(change.detached[0].end, DetachedEnd::Source);
    assert!(!engine.frozen_changes(counter).is_empty());
    assert!(change.removed.data.is_empty());
    let after = engine.export_graph();
    assert!(after.exec_connections.is_empty());
    assert_eq!(after.data_connections, graph.data_connections);
}

#[test]
fn unknown_node_type_survives_load_and_save() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let c = NodeId::generate();
    let graph = GraphModel {
        nodes: vec![
            NodeModel {
                id: a,
                type_name: "ghost/source".into(),
                properties: vec![
                    ("k".into(), number(1)),
                    ("in:3".into(), DataValue::String("x".into())),
                ],
                position: Position { x: 5.0, y: 6.0 },
                pins: None,
                size: None,
            },
            NodeModel {
                id: b,
                type_name: "test/debug".into(),
                properties: vec![("prefix".into(), DataValue::String(String::new()))],
                position: Position::default(),
                pins: None,
                size: None,
            },
            NodeModel {
                id: c,
                type_name: "ghost/sink".into(),
                properties: vec![],
                position: Position::default(),
                pins: None,
                size: None,
            },
        ],
        exec_connections: vec![ConnectionModel {
            src_node: a,
            src_pin: PinId(0),
            dst_node: c,
            dst_pin: PinId(4),
        }],
        data_connections: vec![
            ConnectionModel {
                src_node: a,
                src_pin: PinId(1),
                dst_node: b,
                dst_pin: PinId(1),
            },
            ConnectionModel {
                src_node: a,
                src_pin: PinId(2),
                dst_node: c,
                dst_pin: PinId(5),
            },
        ],
        wire_routes: Vec::new(),
        detached_connections: Vec::new(),
        frames: Vec::new(),
    };
    let mut engine = TickEngine::new();
    assert!(engine.load_graph(&graph).is_empty());
    let mut expected = graph.clone();
    expected.nodes.iter_mut().for_each(|n| {
        n.position = Position::default();
        n.pins = engine.node_model(n.id).unwrap().pins;
    });
    assert_eq!(engine.export_graph().nodes.len(), 3);
    let mut exported = engine.export_graph();
    exported.nodes.sort_by_key(|n| n.id.0);
    expected.nodes.sort_by_key(|n| n.id.0);
    assert_eq!(exported.nodes, expected.nodes);
    assert_eq!(exported.exec_connections, graph.exec_connections);
    let mut want = graph.data_connections.clone();
    want.sort_by_key(|c| (c.src_node.0, c.src_pin.0, c.dst_node.0, c.dst_pin.0));
    assert_eq!(exported.data_connections, want);
    // Pins inferred from connections; the data pin type came from the other end (Debug.Value is Any)
    let pins = engine.node_definition(a).unwrap().pins;
    assert_eq!(pins.len(), 3);
    assert!(pins.iter().any(|p| p.id == PinId(0)
        && p.direction == PinDirection::Output
        && p.pin_type == PinType::Exec));
}

