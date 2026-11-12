use super::super::builtin::MatchNode;
use super::*;

fn inputs(list: &[&str]) -> (String, DataValue) {
    ("inputs".to_string(), cases(list).1)
}

#[test]
fn test_match_cases_and_else() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node(
            id,
            "match",
            &[cases(&[
                "1: t > 25",
                "2: t > 20 and mode = \"auto\"",
                "3: t < 0 or t > 100",
            ])],
        )
        .unwrap();
    let def = engine.node_definition(id).unwrap();
    let inputs: Vec<_> = def
        .pins
        .iter()
        .filter(|p| p.direction == PinDirection::Input)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(inputs, ["In", "mode", "t"]);
    let outputs: Vec<_> = def
        .pins
        .iter()
        .filter(|p| p.direction == PinDirection::Output)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        outputs,
        [
            "t > 25",
            "t > 20 and mode = \"auto\"",
            "t < 0 or t > 100",
            "Else"
        ]
    );

    // Inputs have no values — no condition matches
    assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);

    engine.set_input_value(id, PinId(2), Some(num(30))).unwrap();
    assert_eq!(fire(&mut engine, id), [PinId(1)], "only the first match");

    engine
        .set_node_property(id, "mode", DataValue::from("all"))
        .unwrap();
    engine
        .set_input_value(id, PinId(1), Some(DataValue::String("auto".into())))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(num(120)))
        .unwrap();
    assert_eq!(fire(&mut engine, id), [PinId(1), PinId(2), PinId(3)]);

    engine.set_input_value(id, PinId(2), Some(num(10))).unwrap();
    assert_eq!(fire(&mut engine, id), [MatchNode::ELSE_PIN]);
}

#[test]
fn test_match_rejects_invalid_cases() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node(id, "match", &[cases(&["5: a > 1", "5: a < 0", "0: a = 3"])])
        .unwrap();
    // Duplicate pin numbers and the Else pin are rejected
    assert_eq!(
        engine.node_model(id).unwrap().properties[1],
        cases(&["5: a > 1"])
    );
    // Invalid list is not applied
    engine
        .set_node_property(id, "cases", cases(&["1: a >"]).1)
        .unwrap();
    assert_eq!(
        engine.node_model(id).unwrap().properties[1],
        cases(&["5: a > 1"])
    );
}

#[test]
fn test_match_explicit_inputs_and_types() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine
        .create_node(
            id,
            "match",
            &[
                inputs(&[
                    "1:now:time",
                    "2:limit:time",
                    "3:list:list",
                    "3:dup:any",
                    "4:list:any",
                ]),
                cases(&[
                    "1: now > limit",
                    "2: list contains 3",
                    "3: now >= \"22:00\"",
                ]),
            ],
        )
        .unwrap();
    let def = engine.node_definition(id).unwrap();
    let ins: Vec<_> = def
        .pins
        .iter()
        .filter(|p| p.direction == PinDirection::Input)
        .map(|p| (p.id.0, p.name.as_str()))
        .collect();
    // Duplicate pins and names are rejected
    assert_eq!(ins, [(0, "In"), (1, "now"), (2, "limit"), (3, "list")]);
    assert!(
        def.pins
            .iter()
            .any(|p| p.id == PinId(1) && p.pin_type == PinType::Data(DataType::Time))
    );

    let time = |t: &str| DataValue::Time(ticknode_shared::text::parse_time(t).unwrap());
    engine
        .set_input_value(id, PinId(1), Some(time("22:30")))
        .unwrap();
    engine
        .set_input_value(id, PinId(2), Some(time("23:00")))
        .unwrap();
    // now < limit, list is empty, but now ≥ 22:00 (string is coerced to time)
    assert_eq!(fire(&mut engine, id), [PinId(3)]);
    engine
        .set_input_value(id, PinId(2), Some(time("21:00")))
        .unwrap();
    assert_eq!(fire(&mut engine, id), [PinId(1)]);

    // Explicit inputs survive save and load
    let graph = engine.export_graph();
    let mut copy = TickEngine::new();
    assert!(copy.load_graph(&graph).is_empty());
    assert_eq!(copy.node_definition(id).unwrap().pins, def.pins);
}

#[test]
fn test_match_inputs_are_explicit_after_creation() {
    // A newly created node and the same node after reload react identically to case changes
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "match", &[]).unwrap();
    engine
        .set_node_property(id, "cases", cases(&["1: x > 0"]).1)
        .unwrap();
    let fresh = engine.node_definition(id).unwrap().pins;

    let mut copy = TickEngine::new();
    copy.create_node(id, "match", &[]).unwrap();
    let graph = copy.export_graph();
    let mut copy = TickEngine::new();
    assert!(copy.load_graph(&graph).is_empty());
    copy.set_node_property(id, "cases", cases(&["1: x > 0"]).1)
        .unwrap();
    assert_eq!(copy.node_definition(id).unwrap().pins, fresh);
    // Input `a` remained, `x` did not appear automatically
    assert!(fresh.iter().any(|p| p.name == "a") && !fresh.iter().any(|p| p.name == "x"));
}
