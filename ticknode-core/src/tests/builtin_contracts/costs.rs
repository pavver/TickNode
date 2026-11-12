use super::*;

#[test]
fn catalog_and_restored_nodes_advertise_optional_pins() {
    // These definitions are sent to editors in Init/Catalog and NodeAdded/NodeUpdated.
    // Required is part of that public schema, including on exec and output pins.
    let mut engine = TickEngine::new();
    let definitions = engine.registry.get_definitions();
    assert_eq!(definitions.len(), 35);
    for definition in &definitions {
        assert!(!definition.pins.is_empty(), "{}", definition.type_name);
        for pin in &definition.pins {
            if !((definition.type_name == "variable/set" && pin.id == PinId(3))
                || (matches!(definition.type_name.as_str(), "text/to_text" | "convert")
                    && pin.id == PinId(1))
                || (definition.type_name == "logic/hysteresis" && (1..=3).contains(&pin.id.0)))
            {
                assert!(
                    !pin.required,
                    "catalog: {} / {}",
                    definition.type_name, pin.name
                );
            }
        }
        let properties = if definition.type_name == "object/unpack" {
            vec![property("fields_list", "answer:number,other:string")]
        } else if definition.type_name == "text/template" {
            vec![property("template", "{{ first }}{{ second }}")]
        } else {
            vec![]
        };
        add(&mut engine, &definition.type_name, &properties);
    }
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    for node in &graph.nodes {
        let definition = restored.node_definition(node.id).unwrap();
        assert!(!definition.pins.is_empty());
        if node.type_name == "object/unpack" {
            assert_eq!(definition.pins.len(), 3);
        }
        if node.type_name == "text/template" {
            assert_eq!(definition.pins.len(), 3);
        }
        for pin in &definition.pins {
            if !((node.type_name == "variable/set" && pin.id == PinId(3))
                || (matches!(node.type_name.as_str(), "text/to_text" | "convert")
                    && pin.id == PinId(1))
                || (node.type_name == "logic/hysteresis" && (1..=3).contains(&pin.id.0)))
            {
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
        ("text/template", 0, DataValue::from("")),
        ("compare", 2, DataValue::Bool(true)),
        ("variable/get", 0, number(0)),
        ("list/create", 0, list(&[])),
        ("list/get", 2, number(10)),
        ("list/length", 1, number(0)),
        (
            "object/create",
            0,
            DataValue::Object {
                fields: Default::default(),
                type_name: None,
            },
        ),
        ("object/set_field", 3, object()),
        ("object/get_field", 2, number(42)),
        ("object/unpack", 1, number(42)),
    ] {
        let mut engine = TickEngine::new();
        let properties = if name == "object/unpack" {
            vec![property("fields_list", "answer:number")]
        } else if name == "variable/get" {
            engine
                .variables
                .create(VariableKey::global("answer"), DataType::Number, number(0));
            vec![property("name", "answer"), property("value_type", "number")]
        } else {
            vec![]
        };
        let id = add(&mut engine, name, &properties);
        let mut expected_work = 2; // The pure node and its console consumer.
        if name == "math/div" {
            engine
                .set_input_value(id, PinId(1), Some(number(2)))
                .unwrap();
        }
        if name == "list/get" || name.starts_with("object/") && name != "object/create" {
            let value = if name == "list/get" {
                list(&[10, 20])
            } else {
                object()
            };
            let source = add(&mut engine, "const", &[("value".into(), value)]);
            engine.connect_data(source, PinId(0), id, PinId(0)).unwrap();
            expected_work += 1;
            match name {
                "list/get" => engine
                    .set_input_value(id, PinId(1), Some(number(0)))
                    .unwrap(),
                "object/get_field" | "object/set_field" => {
                    engine
                        .set_input_value(id, PinId(1), Some(DataValue::from("answer")))
                        .unwrap();
                    if name == "object/set_field" {
                        engine
                            .set_input_value(id, PinId(2), Some(number(42)))
                            .unwrap();
                    }
                }
                _ => {}
            }
        }
        let console = add(&mut engine, "console", &[]);
        engine
            .connect_data(id, PinId(pin), console, PinId(1))
            .unwrap();
        activate(&mut engine, console, 0);
        let trace = engine
            .executed
            .iter()
            .find(|trace| trace.node_id == id)
            .unwrap();
        assert!(
            trace.outputs.contains(&(PinId(pin), expected)),
            "{name}: {:?}",
            trace.outputs
        );
        assert_eq!(engine.host_state.logs.len(), 1, "{name}");
        assert_eq!(engine.last_tick.spent, expected_work, "{name}");
    }
}

#[test]
fn active_nodes_charge_one_work_unit_and_produce_effects() {
    for name in [
        "console",
        "test/debug",
        "test/click_button",
        "for_loop",
        "match",
        "sleep",
        "variable/set",
    ] {
        let mut engine = TickEngine::new();
        let properties = if name == "variable/set" {
            vec![property("name", "my_var"), property("value_type", "number")]
        } else {
            vec![]
        };
        let id = add(&mut engine, name, &properties);
        if name == "variable/set" {
            engine
                .variables
                .create(VariableKey::global("my_var"), DataType::Number, 0.into());
            engine
                .set_input_value(id, PinId(3), Some(number(0)))
                .unwrap();
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
                assert_eq!(
                    engine
                        .executed
                        .iter()
                        .find(|trace| trace.node_id == id)
                        .unwrap()
                        .fired,
                    vec![PinId(4)]
                );
            }
            "variable/set" => {
                assert_eq!(
                    engine
                        .variables
                        .get(&VariableKey::global("my_var"))
                        .map(|v| &v.value),
                    Some(&number(0))
                );
                assert_eq!(trace.fired, vec![PinId(1)]);
            }
            _ => panic!("missing expectation for {name}"),
        }
        assert_eq!(engine.last_tick.spent, 1, "{name}");
    }
}
