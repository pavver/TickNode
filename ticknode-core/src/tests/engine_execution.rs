use std::collections::VecDeque;

use super::super::builtin::MatchNode;
use super::*;

struct InputProbe {
    id: NodeId,
    required: bool,
    pure: bool,
}
impl Node for InputProbe {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        self.pure
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/input_probe".into(),
            title: "Input probe".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            properties: vec![],
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "In".into(),
                    direction: PinDirection::Input,
                    pin_type: PinType::Exec,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Value".into(),
                    direction: PinDirection::Input,
                    pin_type: PinType::Data(DataType::Number),
                    required: self.required,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Present".into(),
                    direction: PinDirection::Output,
                    pin_type: PinType::Data(DataType::Bool),
                    required: false,
                },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(
            PinId(2),
            DataValue::Bool(ctx.get_value(PinId(1)).is_some()),
        )]
    }
}

fn inputs(list: &[&str]) -> (String, DataValue) {
    ("inputs".to_string(), cases(list).1)
}

#[test]
fn clear_removes_data_invalidates_pure_cache_and_blocks_required_inputs() {
    let mut engine = TickEngine::new();
    let source = NodeId::generate();
    engine.create_node(source, "for_loop", &[]).unwrap();
    // This test drives output application directly, as plugin emissions do.
    let optional = NodeId::generate();
    let required = NodeId::generate();
    let pure = NodeId::generate();
    for (id, required, pure) in [
        (optional, false, false),
        (required, true, false),
        (pure, false, true),
    ] {
        engine.add_node(Box::new(InputProbe { id, required, pure }));
        engine.connect_data(source, PinId(5), id, PinId(1)).unwrap();
    }
    let mut next = VecDeque::new();
    engine.apply_outputs(
        source,
        vec![],
        vec![NodeOutput::Data(PinId(5), num(7))],
        &mut next,
    );
    assert_eq!(
        engine.pull_data(pure, PinId(2), &mut vec![]),
        Some(DataValue::Bool(true))
    );
    engine.trigger_node(optional, PinId(0));
    engine.trigger_node(required, PinId(0));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == required));
    assert_eq!(
        engine.pull_data(pure, PinId(2), &mut vec![]),
        Some(DataValue::Bool(true))
    );
    engine.apply_outputs(source, vec![], vec![NodeOutput::Clear(PinId(5))], &mut next);
    assert!(!engine.data_cache[&source].contains_key(&PinId(5)));
    assert!(
        !engine
            .executed
            .iter()
            .find(|t| t.node_id == source)
            .unwrap()
            .outputs
            .iter()
            .any(|(p, _)| *p == PinId(5))
    );
    assert_eq!(
        engine.pull_data(pure, PinId(2), &mut vec![]),
        Some(DataValue::Bool(false))
    );
    engine.trigger_node(optional, PinId(0));
    engine.trigger_node(required, PinId(0));
    engine.tick();
    assert!(
        engine
            .executed
            .iter()
            .any(|t| t.node_id == optional && t.inputs == vec![(PinId(1), None)])
    );
    assert!(!engine.executed.iter().any(|t| t.node_id == required));
    assert!(
        engine.host_state.logs.is_empty(),
        "cleared connected sources skip silently"
    );
}

#[test]
fn connections_convert_before_compare_and_match_and_survive_type_changes() {
    let mut engine = TickEngine::new();
    let source = NodeId::generate();
    engine
        .create_node(
            source,
            "const",
            &[("value".into(), DataValue::from("21.5"))],
        )
        .unwrap();
    let compare = NodeId::generate();
    engine.create_node(compare, "compare", &[]).unwrap();
    engine
        .connect_data(source, PinId(0), compare, PinId(0))
        .unwrap();
    engine
        .set_input_value(compare, PinId(1), Some(num(20)))
        .unwrap();
    assert_eq!(
        engine.pull_data(compare, PinId(3), &mut vec![]),
        Some(DataValue::Bool(true))
    );
    let matcher = NodeId::generate();
    engine
        .create_node(
            matcher,
            "match",
            &[
                (
                    "inputs".into(),
                    DataValue::List {
                        element_type: DataType::String,
                        elements: vec![DataValue::from("1:t:number")],
                    },
                ),
                cases(&["1: t > 20"]),
            ],
        )
        .unwrap();
    engine
        .connect_data(source, PinId(0), matcher, PinId(1))
        .unwrap();
    assert_eq!(fire(&mut engine, matcher), vec![PinId(1)]);
    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(fire(&mut restored, matcher), vec![PinId(1)]);
    assert!(
        engine
            .set_node_property(source, "value", num(25))
            .unwrap()
            .data
            .is_empty()
    );
    assert!(
        engine
            .set_node_property(source, "value", DataValue::from("unknown"))
            .unwrap()
            .data
            .is_empty()
    );
    assert_eq!(fire(&mut engine, matcher), vec![MatchNode::ELSE_PIN]);
    let optional = NodeId::generate();
    let required = NodeId::generate();
    for (id, required) in [(optional, false), (required, true)] {
        engine.add_node(Box::new(InputProbe {
            id,
            required,
            pure: false,
        }));
        engine.set_input_value(id, PinId(1), Some(num(99))).unwrap();
        engine.connect_data(source, PinId(0), id, PinId(1)).unwrap();
        engine.trigger_node(id, PinId(0));
    }
    engine.tick();
    assert_eq!(
        engine.data_cache[&optional][&PinId(2)],
        DataValue::Bool(false)
    );
    assert!(!engine.executed.iter().any(|t| t.node_id == required));
    assert_eq!(
        engine
            .host_state
            .logs
            .iter()
            .filter(|(id, m)| *id == required && m.contains("could not be converted"))
            .count(),
        1
    );
    engine.trigger_node(required, PinId(0));
    engine.tick();
    assert_eq!(
        engine
            .host_state
            .logs
            .iter()
            .filter(|(id, _)| *id == required)
            .count(),
        1
    );
    engine
        .set_node_property(source, "value", DataValue::from("22"))
        .unwrap();
    engine.trigger_node(required, PinId(0));
    engine.tick();
    assert!(engine.executed.iter().any(|t| t.node_id == required));
}

#[test]
fn repeated_executions_in_one_tick_share_one_trace() {
    let mut engine = TickEngine::new();
    let first = add_sink(&mut engine);
    let second = add_sink(&mut engine);
    let decision = NodeId::generate();
    engine
        .create_node(decision, "match", &[cases(&["1: t > 0"])])
        .unwrap();
    engine
        .set_input_value(decision, PinId(1), Some(num(5)))
        .unwrap();
    for id in [first, second, first, decision, decision] {
        engine.trigger_node(id, PinId(0));
    }
    engine.tick();
    let ids: Vec<_> = engine.executed.iter().map(|t| t.node_id).collect();
    assert_eq!(ids, [first, second, decision]);
    assert_eq!(engine.executed[2].fired, vec![PinId(1)]);
    // The next tick starts a fresh trace list.
    engine.trigger_node(second, PinId(0));
    engine.tick();
    assert_eq!(
        engine
            .executed
            .iter()
            .map(|t| t.node_id)
            .collect::<Vec<_>>(),
        [second]
    );
}

#[test]
fn test_input_values() {
    let mut engine = TickEngine::new();
    let add = NodeId::generate();
    engine.create_node(add, "math/add", &[]).unwrap();
    engine.set_input_value(add, PinId(0), Some(num(2))).unwrap();
    engine.set_input_value(add, PinId(1), Some(num(3))).unwrap();
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut Vec::new()),
        Some(num(5))
    );

    // Type must match the pin, and the pin must be a data input
    assert!(
        engine
            .set_input_value(add, PinId(0), Some(DataValue::String("x".into())))
            .is_err()
    );
    assert!(engine.set_input_value(add, PinId(2), Some(num(1))).is_err());

    // A connected source takes precedence over an inline value on the node
    let c = NodeId::generate();
    engine
        .create_node(c, "const", &[("value".into(), num(10))])
        .unwrap();
    engine.connect_data(c, PinId(0), add, PinId(0)).unwrap();
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut Vec::new()),
        Some(num(13))
    );
    engine.disconnect_data(add, PinId(0));
    assert_eq!(
        engine.pull_data(add, PinId(2), &mut Vec::new()),
        Some(num(5))
    );

    // Values are saved as properties and restored
    let graph = engine.export_graph();
    let mut copy = TickEngine::new();
    assert!(copy.load_graph(&graph).is_empty());
    assert_eq!(copy.pull_data(add, PinId(2), &mut Vec::new()), Some(num(5)));

    // When a pin disappears after a property change, its value is removed
    let m = NodeId::generate();
    engine
        .create_node(m, "match", &[cases(&["1: a > b"])])
        .unwrap();
    engine.set_input_value(m, PinId(2), Some(num(1))).unwrap();
    engine
        .set_node_property(m, "inputs", inputs(&["1:a:any"]).1)
        .unwrap();
    assert!(
        engine
            .node_model(m)
            .unwrap()
            .properties
            .iter()
            .all(|(n, _)| parse_input_value_property(n).is_none())
    );
    engine.set_input_value(add, PinId(1), None).unwrap();
    assert_eq!(
        engine.node_model(add).unwrap().properties,
        vec![(input_value_property(PinId(0)), num(2))]
    );
}

#[test]
fn test_every_built_in_node_has_help() {
    let engine = TickEngine::new();
    for def in engine.registry.get_definitions() {
        assert!(
            !def.help.trim().is_empty(),
            "node {} has no help",
            def.type_name
        );
    }
}
