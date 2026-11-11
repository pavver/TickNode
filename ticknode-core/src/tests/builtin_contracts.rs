use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, properties: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

fn property(name: &str, value: &str) -> (String, DataValue) {
    (name.into(), DataValue::from(value))
}

fn number(value: i64) -> DataValue { DataValue::from(value) }

fn pull(engine: &mut TickEngine, id: NodeId, pin: u32) -> Option<DataValue> {
    engine.pull_data(id, PinId(pin), &mut Vec::new())
}

fn activate(engine: &mut TickEngine, id: NodeId, pin: u32) -> NodeTrace {
    engine.trigger_node(id, PinId(pin));
    engine.tick();
    engine.executed.iter().find(|trace| trace.node_id == id).unwrap().clone()
}

fn list(elements: &[i64]) -> DataValue {
    DataValue::List { element_type: DataType::Number, elements: elements.iter().copied().map(number).collect() }
}

fn object() -> DataValue {
    DataValue::Object {
        fields: [("answer".into(), ObjectField {
            name: "answer".into(), description: String::new(), value: number(42),
        })].into(),
        type_name: None,
    }
}

#[test]
fn catalog_and_restored_nodes_advertise_optional_pins() {
    // These definitions are sent to editors in Init/Catalog and NodeAdded/NodeUpdated.
    // Required is part of that public schema, including on exec and output pins.
    let mut engine = TickEngine::new();
    let definitions = engine.registry.get_definitions();
    assert_eq!(definitions.len(), 32);
    for definition in &definitions {
        assert!(!definition.pins.is_empty(), "{}", definition.type_name);
        for pin in &definition.pins {
            if !((definition.type_name == "variable/set" && pin.id == PinId(3)) || (definition.type_name == "text/to_text" && pin.id == PinId(1))) {
                assert!(!pin.required, "catalog: {} / {}", definition.type_name, pin.name);
            }
        }
        let properties = if definition.type_name == "object/unpack" {
            vec![property("fields_list", "answer:number,other:string")]
        } else { vec![] };
        add(&mut engine, &definition.type_name, &properties);
    }
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    for node in &graph.nodes {
        let definition = restored.node_definition(node.id).unwrap();
        assert!(!definition.pins.is_empty());
        if node.type_name == "object/unpack" { assert_eq!(definition.pins.len(), 3); }
        for pin in &definition.pins {
            if !((node.type_name == "variable/set" && pin.id == PinId(3)) || (node.type_name == "text/to_text" && pin.id == PinId(1))) {
                assert!(!pin.required, "restored: {} / {}", node.type_name, pin.name);
            }
        }
    }
}

#[test]
fn pure_nodes_charge_one_work_unit_for_their_results() {
    for (name, pin, expected) in [
        ("const", 0, number(0)),
        ("math/add", 2, number(0)),
        ("math/sub", 2, number(0)),
        ("math/mul", 2, number(0)),
        ("math/div", 2, number(0)),
        ("math/expression", 0, DataValue::from(0.0)),
        ("logical/and", 2, DataValue::Bool(false)),
        ("logical/or", 2, DataValue::Bool(false)),
        ("logical/not", 2, DataValue::Bool(true)),
        ("compare", 2, DataValue::Bool(true)),
        ("variable/get", 0, number(0)),
        ("list/create", 0, list(&[])),
        ("list/get", 2, number(10)),
        ("list/length", 1, number(0)),
        ("object/create", 0, DataValue::Object { fields: Default::default(), type_name: None }),
        ("object/set_field", 3, object()),
        ("object/get_field", 2, number(42)),
        ("object/unpack", 1, number(42)),
    ] {
        let mut engine = TickEngine::new();
        let properties = if name == "object/unpack" {
            vec![property("fields_list", "answer:number")]
        } else if name == "variable/get" {
            engine.variables.create(VariableKey::global("answer"), DataType::Number, number(0));
            vec![property("name", "answer"), property("value_type", "number")]
        } else { vec![] };
        let id = add(&mut engine, name, &properties);
        let mut expected_work = 2; // The pure node and its console consumer.
        if name == "math/div" {
            engine.set_input_value(id, PinId(1), Some(number(2))).unwrap();
        }
        if name == "list/get" || name.starts_with("object/") && name != "object/create" {
            let value = if name == "list/get" { list(&[10, 20]) } else { object() };
            let source = add(&mut engine, "const", &[("value".into(), value)]);
            engine.connect_data(source, PinId(0), id, PinId(0)).unwrap();
            expected_work += 1;
            match name {
                "list/get" => engine.set_input_value(id, PinId(1), Some(number(0))).unwrap(),
                "object/get_field" | "object/set_field" => {
                    engine.set_input_value(id, PinId(1), Some(DataValue::from("answer"))).unwrap();
                    if name == "object/set_field" {
                        engine.set_input_value(id, PinId(2), Some(number(42))).unwrap();
                    }
                }
                _ => {}
            }
        }
        let console = add(&mut engine, "console", &[]);
        engine.connect_data(id, PinId(pin), console, PinId(1)).unwrap();
        activate(&mut engine, console, 0);
        let trace = engine.executed.iter().find(|trace| trace.node_id == id).unwrap();
        assert!(trace.outputs.contains(&(PinId(pin), expected)), "{name}: {:?}", trace.outputs);
        assert_eq!(engine.host_state.logs.len(), 1, "{name}");
        assert_eq!(engine.last_tick.spent, expected_work, "{name}");
    }
}

#[test]
fn active_nodes_charge_one_work_unit_and_produce_effects() {
    for name in ["console", "test/debug", "test/click_button", "for_loop", "match", "sleep", "variable/set"] {
        let mut engine = TickEngine::new();
        let properties = if name == "variable/set" {
            vec![property("name", "my_var"), property("value_type", "number")]
        } else {
            vec![]
        };
        let id = add(&mut engine, name, &properties);
        if name == "variable/set" {
            engine.variables.create(VariableKey::global("my_var"), DataType::Number, 0.into());
            engine.set_input_value(id, PinId(3), Some(number(0))).unwrap();
        }
        let trace = activate(&mut engine, id, 0);
        match name {
            "console" => assert_eq!(engine.host_state.logs, vec![(id, "None".into())]),
            "test/debug" => {
                assert_eq!(engine.host_state.logs, vec![(id, "None".into())]);
                assert_eq!(trace.fired, vec![PinId(2)]);
            }
            "test/click_button" | "match" => assert_eq!(trace.fired, vec![PinId(0)]),
            "for_loop" => assert_eq!(trace.fired, vec![PinId(6)]),
            "sleep" => {
                assert_eq!(engine.host_state.active_tasks.len(), 1);
                engine.tick();
                assert_eq!(engine.executed.iter().find(|trace| trace.node_id == id).unwrap().fired, vec![PinId(4)]);
            }
            "variable/set" => {
                assert_eq!(engine.variables.get(&VariableKey::global("my_var")).map(|v| &v.value), Some(&number(0)));
                assert_eq!(trace.fired, vec![PinId(1)]);
            }
            _ => panic!("missing expectation for {name}"),
        }
        assert_eq!(engine.last_tick.spent, 1, "{name}");
    }
}

#[test]
fn arithmetic_and_comparison_use_zero_for_each_missing_operand() {
    for (name, a, b, expected) in [
        ("math/add", Some(7), None, 7),
        ("math/add", None, Some(4), 4),
        ("math/sub", Some(7), Some(4), 3),
        ("math/mul", Some(7), Some(4), 28),
        ("math/div", Some(28), Some(4), 7),
    ] {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, name, &[]);
        engine.set_input_value(id, PinId(0), a.map(number)).unwrap();
        engine.set_input_value(id, PinId(1), b.map(number)).unwrap();
        assert_eq!(pull(&mut engine, id, 2), Some(number(expected)), "{name}");
    }
    let mut engine = TickEngine::new();
    let compare = add(&mut engine, "compare", &[]);
    engine.set_input_value(compare, PinId(0), Some(number(0))).unwrap();
    assert_eq!(pull(&mut engine, compare, 2), Some(DataValue::Bool(true)));
    assert_eq!(pull(&mut engine, compare, 3), Some(DataValue::Bool(false)));
    assert_eq!(pull(&mut engine, compare, 4), Some(DataValue::Bool(false)));
}

#[test]
fn loop_completes_empty_counts_and_does_not_restart_after_completion_or_break() {
    for count in [None, Some(-1), Some(0), Some(1)] {
        let mut engine = TickEngine::new();
        let id = add(&mut engine, "for_loop", &[]);
        engine.set_input_value(id, PinId(1), count.map(number)).unwrap();
        let trace = activate(&mut engine, id, 0);
        if count == Some(1) {
            assert_eq!(trace.fired, vec![PinId(4)]);
            assert_eq!(trace.outputs, vec![(PinId(5), number(0))]);
            assert_eq!(activate(&mut engine, id, 3).fired, vec![PinId(6)]);
        } else {
            assert_eq!(trace.fired, vec![PinId(6)]);
            assert!(trace.outputs.is_empty());
        }
        for pin in [3, 2] { assert!(activate(&mut engine, id, pin).fired.is_empty()); }
        engine.set_input_value(id, PinId(1), Some(number(3))).unwrap();
        assert_eq!(activate(&mut engine, id, 0).fired, vec![PinId(4)]);
        assert_eq!(activate(&mut engine, id, 2).fired, vec![PinId(6)]);
        for pin in [3, 2] { assert!(activate(&mut engine, id, pin).fired.is_empty()); }
        // Starting an empty loop also cancels a previously active loop.
        assert_eq!(activate(&mut engine, id, 0).fired, vec![PinId(4)]);
        engine.set_input_value(id, PinId(1), Some(number(0))).unwrap();
        assert_eq!(activate(&mut engine, id, 0).fired, vec![PinId(6)]);
        assert!(activate(&mut engine, id, 3).fired.is_empty());
    }
}

#[test]
fn list_length_defaults_to_zero_and_uses_the_public_length_pin() {
    let mut engine = TickEngine::new();
    let length = add(&mut engine, "list/length", &[]);
    assert_eq!(pull(&mut engine, length, 1), Some(number(0)));
    let source = add(&mut engine, "const", &[("value".into(), list(&[10, 20, 30]))]);
    engine.connect_data(source, PinId(0), length, PinId(0)).unwrap();
    assert_eq!(pull(&mut engine, length, 1), Some(number(3)));
}

#[test]
fn clock_routes_zero_pin_and_charges_polling_and_activation() {
    for interval in [1, 3, 0, -1] {
        let mut engine = TickEngine::new();
        let clock = add(&mut engine, "clock", &[("interval_ticks".into(), number(interval))]);
        let console = add(&mut engine, "console", &[]);
        engine.connect_exec(clock, PinId(0), console, PinId(0)).unwrap();
        let mut pulses = Vec::new();
        let mut deliveries = Vec::new();
        for tick in 1..=7 {
            engine.tick();
            let fired = engine.executed.iter().find(|trace| trace.node_id == clock);
            if let Some(trace) = fired {
                assert_eq!(trace.fired, vec![PinId(0)]);
                pulses.push(tick);
            }
            let delivered = engine.executed.iter().any(|trace| trace.node_id == console);
            if delivered { deliveries.push(tick); }
            assert_eq!(engine.last_tick.spent, 1 + u32::from(fired.is_some()) + u32::from(delivered));
        }
        assert_eq!(pulses, if interval == 3 { vec![3, 6] } else { vec![1, 2, 3, 4, 5, 6, 7] });
        assert_eq!(deliveries, if interval == 3 { vec![4, 7] } else { vec![2, 3, 4, 5, 6, 7] });
        assert_eq!(engine.host_state.logs.len(), deliveries.len());
    }
}

#[test]
fn clock_with_zero_interval_still_pulses_every_tick() {
    let mut engine = TickEngine::new();
    let clock = NodeId::generate();
    // Native callers can construct a source before any property setter has run.
    engine.add_node(Box::new(ClockNode { id: clock, interval_ticks: 0 }));
    let console = add(&mut engine, "console", &[]);
    engine.connect_exec(clock, PinId(0), console, PinId(0)).unwrap();
    for _ in 0..3 {
        engine.tick();
        assert_eq!(engine.executed.iter().find(|trace| trace.node_id == clock).unwrap().fired, vec![PinId(0)]);
    }
    assert_eq!(engine.host_state.logs, vec![(console, "None".into()), (console, "None".into())]);
}

#[test]
fn sleep_duration_and_tick_values_deliver_on_the_expected_tick() {
    for (delay, expected_ticks) in [
        (None, 1),
        (Some(DataValue::Ticks(0)), 1),
        (Some(DataValue::Ticks(1)), 1),
        (Some(DataValue::Ticks(3)), 3),
        (Some(DataValue::Duration(std::time::Duration::ZERO)), 1),
        (Some(DataValue::Duration(std::time::Duration::from_nanos(1))), 1),
        (Some(DataValue::Duration(std::time::Duration::from_millis(250))), 3),
    ] {
        let mut engine = TickEngine::new();
        let sleep = add(&mut engine, "sleep", &[]);
        let sink = add(&mut engine, "console", &[]);
        engine.connect_exec(sleep, PinId(4), sink, PinId(0)).unwrap();
        engine.set_input_value(sleep, PinId(1), delay).unwrap();
        assert!(activate(&mut engine, sleep, 0).fired.is_empty());
        assert_eq!(engine.host_state.active_tasks.len(), 1);
        for elapsed in 1..=expected_ticks {
            engine.tick();
            let delivered = engine.executed.iter().find(|trace| trace.node_id == sleep);
            if elapsed == expected_ticks {
                assert_eq!(delivered.unwrap().fired, vec![PinId(4)]);
                assert!(engine.host_state.active_tasks.is_empty());
                assert!(engine.execution_queue.contains(&(sink, PinId(0))));
            } else {
                assert!(delivered.is_none());
                assert_eq!(engine.host_state.active_tasks.len(), 1);
            }
        }
        engine.tick();
        assert_eq!(engine.host_state.logs, vec![(sink, "None".into())]);
    }
}

#[test]
fn variable_defaults_write_and_read_zero_and_forward_execution() {
    let mut engine = TickEngine::new();
    engine.variables.create(VariableKey::global("answer"), DataType::Number, number(0));
    let get = add(&mut engine, "variable/get", &[property("var_name", "answer")]);
    let set = add(&mut engine, "variable/set", &[property("var_name", "answer")]);
    let sink = add(&mut engine, "console", &[]);
    engine.connect_exec(set, PinId(1), sink, PinId(0)).unwrap();
    assert_eq!(pull(&mut engine, get, 0), Some(number(0)));
    engine.set_input_value(set, PinId(3), Some(number(42))).unwrap();
    assert_eq!(activate(&mut engine, set, 0).fired, vec![PinId(1)]);
    assert_eq!(engine.variables.get(&VariableKey::global("answer")).map(|v| &v.value), Some(&number(42)));
    assert_eq!(pull(&mut engine, get, 0), Some(number(42)));
    engine.tick();
    assert_eq!(engine.host_state.logs, vec![(sink, "None".into())]);
    engine.set_input_value(set, PinId(3), Some(number(0))).unwrap();
    assert_eq!(activate(&mut engine, set, 0).fired, vec![PinId(1)]);
    assert_eq!(engine.variables.get(&VariableKey::global("answer")).map(|v| &v.value), Some(&number(0)));
    assert_eq!(pull(&mut engine, get, 0), Some(number(0)));
}

#[test]
fn debug_prefix_and_button_label_survive_edits_and_graph_restoration() {
    let mut engine = TickEngine::new();
    let button = add(&mut engine, "test/click_button", &[property("label", "Run automation")]);
    let debug = add(&mut engine, "test/debug", &[property("prefix", "Value: ")]);
    engine.connect_exec(button, PinId(0), debug, PinId(0)).unwrap();
    for (id, key, value) in [(button, "label", "Start"), (debug, "prefix", "Result: ")] {
        engine.set_node_property(id, key, DataValue::from(value)).unwrap();
        engine.set_node_property(id, "unrelated", DataValue::from("ignored")).unwrap();
        assert!(engine.node_model(id).unwrap().properties.contains(&property(key, value)));
    }
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&engine.export_graph()).is_empty());
    assert!(restored.node_model(button).unwrap().properties.contains(&property("label", "Start")));
    assert_eq!(activate(&mut restored, button, 0).fired, vec![PinId(0)]);
    restored.tick();
    assert_eq!(restored.host_state.logs, vec![(debug, "Result: None".into())]);
    assert_eq!(restored.executed.iter().find(|trace| trace.node_id == debug).unwrap().fired, vec![PinId(2)]);
    restored.set_input_value(debug, PinId(1), Some(number(42))).unwrap();
    assert_eq!(activate(&mut restored, debug, 0).fired, vec![PinId(2)]);
    assert_eq!(restored.host_state.logs, vec![(debug, "Result: None".into()), (debug, "Result: 42".into())]);
}
