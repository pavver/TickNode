#[test]
fn split_plugin_struct_pins_flow_persist_and_survive_disable() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let source = NodeId::generate();
    let sink = NodeId::generate();
    engine
        .create_node(
            source,
            "test/point_source",
            &[("x".into(), number(5)), ("y".into(), number(7))],
        )
        .unwrap();
    engine.create_node(sink, "test/point_sink", &[]).unwrap();
    engine
        .connect_data(source, PinId(1), sink, PinId(1))
        .unwrap();
    assert_eq!(
        engine
            .set_pin_split(source, PinId(1), true)
            .unwrap()
            .data
            .len(),
        1
    );
    engine.set_pin_split(sink, PinId(1), true).unwrap();
    for i in 0..3 {
        let field = split_field_pin(PinId(1), i).unwrap();
        engine.connect_data(source, field, sink, field).unwrap();
    }
    engine.trigger_node(source, PinId(0));
    engine.trigger_node(sink, PinId(0));
    // Explicit ordering: let the source's plugin output arrive before pulling the sink's inputs.
    engine.tick();
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 4), Some(number(12)));
    let trace = engine.executed.iter().find(|t| t.node_id == sink).unwrap();
    assert!(
        trace
            .inputs
            .iter()
            .any(|(p, v)| *p == split_field_pin(PinId(1), 2).unwrap() && v.is_none())
    );
    let graph = engine.export_graph();
    let before = engine.node_definition(source).unwrap();
    engine.disable_plugin("test_plugin").unwrap();
    assert_eq!(engine.export_graph(), graph);
    assert_eq!(engine.node_definition(source).unwrap(), before);
    assert!(engine.set_pin_split(source, PinId(1), false).is_err());
    assert!(engine.load_graph(&graph).is_empty());
    assert_eq!(engine.export_graph(), graph);
    engine.enable_plugin("test_plugin", &[]).unwrap();
    assert_eq!(engine.export_graph(), graph);
    assert!(engine.load_graph(&graph).is_empty());
    let snippet = engine.export_snippet(&[source, sink]);
    let (map, warnings) = engine.import_snippet(&snippet);
    assert!(warnings.is_empty());
    let source = map.iter().find(|(old, _)| *old == source).unwrap().1;
    let sink = map.iter().find(|(old, _)| *old == sink).unwrap().1;
    engine.trigger_node(source, PinId(0));
    engine.tick();
    engine.trigger_node(sink, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, sink, 4), Some(number(12)));
    assert_eq!(
        engine
            .set_pin_split(sink, PinId(1), false)
            .unwrap()
            .data
            .len(),
        3
    );
}

#[test]
fn plugin_cost_tracks_properties_and_controls_budget() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let id = NodeId::generate();
    engine
        .create_node(id, "test/cost", &[("cost".into(), number(7))])
        .unwrap();
    assert_eq!(engine.node_definition(id).unwrap().cost, 7);
    engine.budget.limit = Some(3);
    engine.trigger_node(id, PinId(0));
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(
        engine.last_tick,
        TickStats {
            spent: 7,
            deferred: 1
        }
    );
    engine.set_node_property(id, "cost", number(2)).unwrap();
    assert_eq!(engine.node_definition(id).unwrap().cost, 2);
    engine.tick();
    assert_eq!(
        engine.last_tick,
        TickStats {
            spent: 2,
            deferred: 0
        }
    );
    let graph = engine.export_graph();
    assert!(engine.load_graph(&graph).is_empty());
    assert_eq!(engine.node_definition(id).unwrap().cost, 2);
}

#[test]
fn core_c_host_clock_and_reserved_pins_are_visible_through_the_catalog_and_execution() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    engine
        .enable_plugin("test_plugin", &test_settings("clock", "t"))
        .unwrap();
    let sets = engine.property_options();
    assert_eq!(option_set(&sets, "clock").unwrap().options[0].value, "0:0");
    let definition = engine
        .registry
        .get_definition("test/clock")
        .unwrap()
        .clone();
    assert_eq!(
        definition.pins.iter().map(|p| p.id).collect::<Vec<_>>(),
        [PinId(0), PinId(1), PinId(2)]
    );
    let node = NodeId::generate();
    engine.create_node(node, "test/clock", &[]).unwrap();
    assert_eq!(engine.node_definition(node).unwrap().pins, definition.pins);
    engine.host_state.tick_duration = std::time::Duration::from_millis(37);
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(output(&engine, node, 1), Some(number(1)));
    assert_eq!(output(&engine, node, 2), Some(number(37)));
    let trace = engine.executed.iter().find(|t| t.node_id == node).unwrap();
    assert_eq!(
        trace.outputs,
        [(PinId(1), number(1)), (PinId(2), number(37))]
    );
}

#[test]
fn core_c_editing_existing_plugin_properties_survives_instance_recreation() {
    let Some(mut engine) = engine_with("test_plugin", vec![]) else {
        return;
    };
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "test/point_source",
            &[("x".into(), number(2)), ("y".into(), number(3))],
        )
        .unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(id, log)| *id == crash && log.contains("Plugin crashed"))
    );
    // Edit before the next tick recreates the resource from the stored properties.
    engine.set_node_property(node, "y", number(9)).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    let Some(DataValue::Object { fields, .. }) = output(&engine, node, 1) else {
        panic!("missing point")
    };
    assert_eq!(fields["x"].value, DataValue::from(2.0));
    assert_eq!(fields["y"].value, DataValue::from(9.0));
    let properties = engine.node_model(node).unwrap().properties;
    assert_eq!(properties.iter().filter(|(key, _)| key == "y").count(), 1);
    assert!(properties.contains(&("y".into(), DataValue::from(9.0))));
}

#[test]
fn delay_queue_timers_and_sdk_time_rounding() {
    let Some(mut engine) = engine_with("delay_plugin", vec![]) else {
        return;
    };
    let node = NodeId::generate();
    engine.create_node(node, "delay/queue", &[]).unwrap();
    engine
        .set_input_value(node, PinId(2), Some(DataValue::Ticks(5)))
        .unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    engine
        .set_input_value(
            node,
            PinId(2),
            Some(DataValue::Duration(
                std::time::Duration::from_millis(100) + std::time::Duration::from_nanos(1),
            )),
        )
        .unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    // The second task rounds up to two ticks, but the older, longer task supplies progress.
    assert_eq!(
        engine.delay_timers(),
        vec![NodeTimer {
            node,
            total_ms: 500,
            remaining_ms: 400,
            count: 2
        }]
    );
    engine.tick();
    assert_eq!(engine.delay_timers()[0].count, 2);
    engine.tick();
    assert_eq!(engine.delay_timers()[0].count, 1);
    engine.tick();
    engine.tick();
    assert!(engine.delay_timers().is_empty());
    engine
        .set_input_value(
            node,
            PinId(2),
            Some(DataValue::Duration(std::time::Duration::from_nanos(1))),
        )
        .unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(engine.delay_timers()[0].total_ms, 100);
    engine.tick();
    assert!(engine.delay_timers().is_empty());
}
