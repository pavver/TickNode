use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, props: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, props).unwrap();
    id
}

fn setup(mode: &str, on: i64, off: i64) -> (TickEngine, NodeId) {
    let mut engine = TickEngine::new();
    let id = add(
        &mut engine,
        "logic/hysteresis",
        &[("mode".into(), mode.into())],
    );
    for (pin, value) in [(2, on), (3, off)] {
        engine
            .set_input_value(id, PinId(pin), Some(value.into()))
            .unwrap();
    }
    (engine, id)
}

fn activate(engine: &mut TickEngine, id: NodeId) -> NodeTrace {
    engine.trigger_node(id, PinId(0));
    engine.tick();
    engine
        .executed
        .iter()
        .find(|t| t.node_id == id)
        .unwrap()
        .clone()
}

fn evaluate(engine: &mut TickEngine, id: NodeId, value: i64, state: bool, transition: Option<u32>) {
    engine
        .set_input_value(id, PinId(1), Some(value.into()))
        .unwrap();
    let trace = activate(engine, id);
    assert_eq!(trace.outputs, vec![(PinId(7), state.into())]);
    let mut expected: Vec<_> = transition.into_iter().map(PinId).collect();
    expected.push(PinId(6));
    assert_eq!(trace.fired, expected, "value {value}");
}

#[test]
fn rising_transitions_keep_state_in_dead_band_and_at_boundaries() {
    let (mut engine, id) = setup("rising", 230, 200);
    for (value, state, transition) in [
        (215, false, None),
        (230, false, None),
        (231, true, Some(4)),
        (240, true, None),
        (230, true, None),
        (215, true, None),
        (200, true, None),
        (199, false, Some(5)),
        (190, false, None),
        (200, false, None),
        (215, false, None),
        (231, true, Some(4)),
    ] {
        evaluate(&mut engine, id, value, state, transition);
        assert_eq!(engine.last_tick.spent, 1);
    }
}

#[test]
fn falling_transitions_keep_state_in_dead_band_and_at_boundaries() {
    let (mut engine, id) = setup("falling", 19, 21);
    for (value, state, transition) in [
        (20, false, None),
        (19, false, None),
        (18, true, Some(4)),
        (17, true, None),
        (19, true, None),
        (20, true, None),
        (21, true, None),
        (22, false, Some(5)),
        (23, false, None),
        (21, false, None),
        (20, false, None),
        (18, true, Some(4)),
    ] {
        evaluate(&mut engine, id, value, state, transition);
    }
}

#[test]
fn equal_thresholds_are_valid_and_equality_keeps_state() {
    for mode in ["rising", "falling"] {
        let (mut engine, id) = setup(mode, 10, 10);
        let (on, off) = if mode == "rising" { (11, 9) } else { (9, 11) };
        evaluate(&mut engine, id, 10, false, None);
        evaluate(&mut engine, id, on, true, Some(4));
        evaluate(&mut engine, id, 10, true, None);
        evaluate(&mut engine, id, off, false, Some(5));
        assert!(engine.node_errors().is_empty());
    }
}

#[test]
fn value_connection_adapts_all_numeric_pins_and_preserves_inline_thresholds() {
    let (mut engine, id) = setup("rising", 230, 200);
    let source = add(
        &mut engine,
        "const",
        &[
            ("value_type".into(), "quantity.voltage".into()),
            ("value".into(), 231i64.into()),
        ],
    );
    engine.connect_data(source, PinId(0), id, PinId(1)).unwrap();
    let def = engine.node_definition(id).unwrap();
    assert_eq!(
        (&*def.title, &*def.category, def.cost, def.adaptive),
        ("Hysteresis", "Logic", 1, true)
    );
    for pin in def.pins.iter().filter(|p| (1..=3).contains(&p.id.0)) {
        assert_eq!(
            pin.pin_type,
            PinType::Data(DataType::Named("quantity.voltage".into()))
        );
        assert!(pin.required);
    }
    let props = engine.node_model(id).unwrap().properties;
    for (pin, value) in [(2, 230i64), (3, 200i64)] {
        assert!(props.contains(&(input_value_property(PinId(pin)), value.into())));
    }
    let trace = activate(&mut engine, id);
    assert_eq!(trace.outputs, vec![(PinId(7), true.into())]);
    assert_eq!(trace.fired, vec![PinId(4), PinId(6)]);
    engine
        .set_node_property(source, "value", 199i64.into())
        .unwrap();
    assert_eq!(activate(&mut engine, id).fired, vec![PinId(5), PinId(6)]);
}

#[test]
fn only_numeric_types_adapt_and_only_value_adapts_on_connect() {
    let mut node = HysteresisNode::new(NodeId::generate());
    assert!(!node.is_pure());
    assert!(!node.polls());
    for pin in 0..=7 {
        assert_eq!(node.adapts_on_connect(PinId(pin)), pin == 1);
        assert!(!node.adapt_to(PinId(pin), &DataType::String));
    }
    for value in ["bool", "string", "ha.state", "quantity.unknown"] {
        node.set_property("value_type", value.into());
        assert!(
            node.properties()
                .contains(&("value_type".into(), "number".into()))
        );
    }
    node.set_property("mode", "unknown".into());
    assert!(
        node.properties()
            .contains(&("mode".into(), "rising".into()))
    );
}

#[test]
fn invalid_thresholds_keep_state_and_out_and_log_once_until_recovery() {
    for (mode, on, off, invalid_off, value) in
        [("rising", 230, 200, 240, 250), ("falling", 19, 21, 18, 17)]
    {
        for initial in [false, true] {
            let (mut engine, id) = setup(mode, on, off);
            if initial {
                evaluate(&mut engine, id, value, true, Some(4));
            }
            engine
                .set_input_value(id, PinId(1), Some(value.into()))
                .unwrap();
            let source = add(
                &mut engine,
                "const",
                &[("value".into(), invalid_off.into())],
            );
            engine.connect_data(source, PinId(0), id, PinId(3)).unwrap();
            for _ in 0..3 {
                let trace = activate(&mut engine, id);
                assert_eq!(trace.outputs, vec![(PinId(7), initial.into())]);
                assert_eq!(trace.fired, vec![PinId(6)]);
                let errors = engine.node_errors();
                assert_eq!(errors.len(), 1);
                assert_eq!(errors[0].message.key, "error-hysteresis-thresholds");
                assert!(!errors[0].message.fallback.is_empty());
                assert_eq!(engine.host_state.logs.len(), 1);
                assert!(
                    engine.host_state.logs[0]
                        .1
                        .starts_with("[ERROR] Invalid hysteresis thresholds")
                );
            }
            engine
                .set_node_property(source, "value", off.into())
                .unwrap();
            let trace = activate(&mut engine, id);
            assert_eq!(trace.outputs, vec![(PinId(7), true.into())]);
            assert!(engine.node_errors().is_empty());
            engine
                .set_node_property(source, "value", invalid_off.into())
                .unwrap();
            assert_eq!(activate(&mut engine, id).fired, vec![PinId(6)]);
            assert_eq!(engine.host_state.logs.len(), 2);
            assert_eq!(engine.node_errors().len(), 1);
        }
    }
}

#[test]
fn required_inputs_block_evaluation() {
    for missing in 1..=3 {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, "logic/hysteresis", &[]);
        for pin in (1..=3).filter(|p| *p != missing) {
            engine
                .set_input_value(id, PinId(pin), Some(0i64.into()))
                .unwrap();
        }
        engine.trigger_node(id, PinId(0));
        engine.tick();
        assert!(engine.executed.is_empty());
        assert!(engine.execution_queue.is_empty());
        assert_eq!(
            engine.node_errors()[0].message.key,
            "error-node-required-missing"
        );
    }
}

#[test]
fn out_runs_after_transition_and_downstream_reads_the_new_state() {
    let (mut engine, id) = setup("rising", 230, 200);
    let turned_on = add(&mut engine, "console", &[]);
    let out = add(&mut engine, "console", &[]);
    for (pin, target) in [(4, turned_on), (6, out)] {
        engine
            .connect_exec(id, PinId(pin), target, PinId(0))
            .unwrap();
        engine.connect_data(id, PinId(7), target, PinId(1)).unwrap();
    }
    evaluate(&mut engine, id, 231, true, Some(4));
    assert_eq!(
        engine.execution_queue.iter().copied().collect::<Vec<_>>(),
        vec![(turned_on, PinId(0)), (out, PinId(0))]
    );
    engine.tick();
    assert_eq!(
        engine.host_state.logs,
        vec![(turned_on, "true".into()), (out, "true".into())]
    );
    evaluate(&mut engine, id, 215, true, None);
    assert_eq!(
        engine.execution_queue.iter().copied().collect::<Vec<_>>(),
        vec![(out, PinId(0))]
    );
}

#[test]
fn graph_roundtrip_preserves_mode_type_and_thresholds_but_resets_state() {
    for (mode, on, off, value) in [("rising", 230, 200, 231), ("falling", 200, 230, 199)] {
        let (mut engine, id) = setup(mode, on, off);
        engine
            .set_node_property(id, "value_type", "quantity.voltage".into())
            .unwrap();
        evaluate(&mut engine, id, value, true, Some(4));
        engine
            .set_input_value(id, PinId(1), Some(215i64.into()))
            .unwrap();
        let graph = engine.export_graph();
        let mut restored = TickEngine::new();
        assert!(restored.load_graph(&graph).is_empty());
        assert_eq!(
            restored.node_model(id).unwrap().properties,
            engine.node_model(id).unwrap().properties
        );
        let props = restored.node_model(id).unwrap().properties;
        assert!(props.contains(&("mode".into(), mode.into())));
        assert!(props.contains(&("value_type".into(), "quantity.voltage".into())));
        let trace = activate(&mut restored, id);
        assert_eq!(trace.outputs, vec![(PinId(7), false.into())]);
        assert_eq!(trace.fired, vec![PinId(6)]);
    }
}
