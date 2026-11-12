use super::*;

#[test]
fn test_graph_roundtrip_keeps_properties_and_connections() {
    let mut engine = TickEngine::new();
    let a = NodeId::generate();
    let b = NodeId::generate();
    let dbg = NodeId::generate();
    engine
        .create_node(
            a,
            "const",
            &[("value".into(), DataValue::String("hi".into()))],
        )
        .unwrap();
    engine
        .create_node(
            b,
            "math/expression",
            &[("expression".into(), DataValue::String("x * 2".into()))],
        )
        .unwrap();
    engine
        .create_node(
            dbg,
            "test/debug",
            &[("prefix".into(), DataValue::String("> ".into()))],
        )
        .unwrap();
    engine.connect_data(a, PinId(0), dbg, PinId(1)).unwrap();

    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    let warnings = restored.load_graph(&graph);
    assert!(warnings.is_empty(), "{:?}", warnings);
    assert_eq!(restored.export_graph(), graph);
    // Formula pin was restored from the expression
    let def = restored.node_definition(b).unwrap();
    assert!(
        def.pins
            .iter()
            .any(|p| p.name == "x" && p.direction == PinDirection::Input)
    );
}

#[test]
fn test_data_input_has_single_source_and_exec_is_validated() {
    let mut engine = TickEngine::new();
    let c1 = NodeId::generate();
    let c2 = NodeId::generate();
    let dbg = add_sink(&mut engine);
    engine.create_node(c1, "const", &[]).unwrap();
    engine.create_node(c2, "const", &[]).unwrap();
    engine.connect_data(c1, PinId(0), dbg, PinId(1)).unwrap();
    engine.connect_data(c2, PinId(0), dbg, PinId(1)).unwrap();
    assert_eq!(engine.data_connections().len(), 1);
    assert_eq!(engine.data_connections()[0].src_node, c2);

    // Data pin cannot be connected as exec
    assert_eq!(
        engine.connect_exec(c1, PinId(0), dbg, PinId(0)),
        Err(EngineError::InvalidPinConnection)
    );
}

#[test]
fn test_property_change_prunes_invalid_connections() {
    let mut engine = TickEngine::new();
    let x = NodeId::generate();
    let expr = NodeId::generate();
    engine.create_node(x, "const", &[]).unwrap();
    engine
        .create_node(
            expr,
            "math/expression",
            &[("expression".into(), DataValue::String("a + b".into()))],
        )
        .unwrap();
    engine.connect_data(x, PinId(0), expr, PinId(2)).unwrap(); // b

    // Formula now has only one variable — pin 2 no longer exists
    let removed = engine
        .set_node_property(expr, "expression", DataValue::String("a * 3".into()))
        .unwrap();
    assert_eq!(removed.data.len(), 1);
    assert!(engine.data_connections().is_empty());
}

#[test]
fn test_remove_node_cancels_its_tasks() {
    let mut engine = TickEngine::new();
    let sleep = NodeId::generate();
    engine.create_node(sleep, "sleep", &[]).unwrap();
    let delay = NodeId::generate();
    engine
        .create_node(delay, "const", &[("value".into(), DataValue::Ticks(100))])
        .unwrap();
    engine
        .connect_data(delay, PinId(0), sleep, PinId(1))
        .unwrap();
    engine.trigger_node(sleep, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);

    engine.remove_node(sleep);
    assert!(engine.host_state.active_tasks.is_empty());
}

#[test]
fn test_clock_fires_every_interval() {
    let mut engine = TickEngine::new();
    let clock = NodeId::generate();
    engine
        .create_node(
            clock,
            "clock",
            &[(
                "interval_ticks".into(),
                DataValue::Number(NumberValue::Int(3)),
            )],
        )
        .unwrap();
    let sink = add_sink(&mut engine);
    engine
        .connect_exec(clock, PinId(0), sink, PinId(0))
        .unwrap();

    let mut fired_on = Vec::new();
    for _ in 0..9 {
        engine.tick();
        if engine.executed.iter().any(|t| t.node_id == clock) {
            fired_on.push(engine.host_state.current_tick);
        }
    }
    assert_eq!(fired_on, vec![3, 6, 9]);
}

#[test]
fn test_traces_record_inputs_outputs_and_fired_pins() {
    let mut engine = TickEngine::new();
    let num = |v| DataValue::Number(NumberValue::Int(v));
    let (a, b, add, debug) = (
        NodeId::generate(),
        NodeId::generate(),
        NodeId::generate(),
        NodeId::generate(),
    );
    engine
        .create_node(a, "const", &[("value".into(), num(2))])
        .unwrap();
    engine
        .create_node(b, "const", &[("value".into(), num(3))])
        .unwrap();
    engine.create_node(add, "math/add", &[]).unwrap();
    engine.create_node(debug, "test/debug", &[]).unwrap();
    engine.connect_data(a, PinId(0), add, PinId(0)).unwrap();
    engine.connect_data(b, PinId(0), add, PinId(1)).unwrap();
    engine.connect_data(add, PinId(2), debug, PinId(1)).unwrap();

    engine.trigger_node(debug, PinId(0));
    engine.tick();

    let trace = |id| {
        engine
            .executed
            .iter()
            .find(|t| t.node_id == id)
            .cloned()
            .unwrap()
    };
    let d = trace(debug);
    assert_eq!(d.tick, 1);
    assert_eq!(d.inputs, vec![(PinId(1), Some(num(5)))]);
    assert_eq!(d.fired, vec![PinId(2)]);
    // Pure node evaluated on demand also produces a trace.
    let s = trace(add);
    assert_eq!(
        s.inputs,
        vec![(PinId(0), Some(num(2))), (PinId(1), Some(num(3)))]
    );
    assert_eq!(s.outputs, vec![(PinId(2), num(5))]);

    engine.tick();
    assert!(engine.executed.is_empty());
}

#[test]
fn test_import_snippet_maps_ids_and_keeps_inner_connections() {
    let mut engine = TickEngine::new();
    let (a, b, outside) = (NodeId::generate(), NodeId::generate(), NodeId::generate());
    engine.create_node(a, "const", &[]).unwrap();
    engine.create_node(b, "test/debug", &[]).unwrap();
    engine.create_node(outside, "test/debug", &[]).unwrap();
    engine.connect_data(a, PinId(0), b, PinId(1)).unwrap();
    engine.connect_data(a, PinId(0), outside, PinId(1)).unwrap();
    let mut snippet = engine.export_snippet(&[a, b]);
    // Connection to a node outside the snippet (e.g., from another graph's clipboard) is ignored.
    snippet.data_connections.push(ConnectionModel {
        src_node: a,
        src_pin: PinId(0),
        dst_node: outside,
        dst_pin: PinId(1),
    });

    let (created, warnings) = engine.import_snippet(&snippet);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        created.iter().map(|(old, _)| *old).collect::<Vec<_>>(),
        vec![a, b]
    );
    let (new_a, new_b) = (created[0].1, created[1].1);
    assert!(new_a != a && new_b != b);
    assert_eq!(
        engine
            .export_snippet(&[new_a, new_b])
            .data_connections
            .len(),
        1
    );
    assert_eq!(engine.node_ids().len(), 5);
}
