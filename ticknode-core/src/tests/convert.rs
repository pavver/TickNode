use crate::{Node, TickEngine, builtin};
use ticknode_shared::*;

fn add(e: &mut TickEngine, name: &str, props: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    e.create_node(id, name, props).unwrap();
    id
}
fn prop(k: &str, v: &str) -> (String, DataValue) {
    (k.into(), v.into())
}
fn pull(e: &mut TickEngine, id: NodeId) -> Option<DataValue> {
    e.pull_data(id, PinId(0), &mut vec![])
}
fn output(e: &TickEngine, id: NodeId) -> DataType {
    let PinType::Data(t) = e
        .node_definition(id)
        .unwrap()
        .pins
        .iter()
        .find(|p| p.id == PinId(0))
        .unwrap()
        .pin_type
        .clone()
    else {
        panic!()
    };
    t
}
fn q(name: &str) -> DataType {
    DataType::Named(format!("quantity.{name}"))
}
fn stored(e: &TickEngine, id: NodeId, key: &str) -> DataValue {
    e.node_model(id)
        .unwrap()
        .properties
        .iter()
        .find(|(k, _)| k == key)
        .unwrap()
        .1
        .clone()
}

#[test]
fn connect_adapts_input_and_switches_unavailable_target_to_first_choice() {
    let mut e = TickEngine::new();
    let node = add(&mut e, "convert", &[]);
    assert_eq!(output(&e, node), DataType::Any);
    assert_eq!(pull(&mut e, node), None);
    let power = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.power"),
            ("value".into(), 2000i64.into()),
        ],
    );
    e.connect_data(power, PinId(0), node, PinId(1)).unwrap();
    assert_eq!(stored(&e, node, "value_type"), "quantity.power".into());
    assert_eq!(stored(&e, node, "target"), "number".into());
    assert_eq!(stored(&e, node, "unit"), "watt".into());
    assert_eq!(pull(&mut e, node), Some(2000i64.into()));
    e.set_node_property(node, "target", "signal_strength".into())
        .unwrap();
    assert_eq!(output(&e, node), q("signal_strength"));
    let wind = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.speed"),
            ("value".into(), 6.688.into()),
        ],
    );
    e.connect_data(wind, PinId(0), node, PinId(1)).unwrap();
    assert_eq!(stored(&e, node, "target"), "number".into());
    assert_eq!(
        stored(&e, node, "unit"),
        quantity::kind("quantity.speed")
            .unwrap()
            .canonical
            .clone()
            .into()
    );
    e.set_node_property(node, "target", "beaufort".into())
        .unwrap();
    assert_eq!(output(&e, node), q("beaufort"));
    assert_eq!(pull(&mut e, node), Some(4i64.into()));
    assert!(
        !e.node_definition(node)
            .unwrap()
            .properties
            .iter()
            .any(|p| p.name == "unit")
    );
    e.adapt_node(node, PinId(1), &DataType::DateTime).unwrap();
    assert_eq!(stored(&e, node, "target"), "time".into());
    assert_eq!(output(&e, node), DataType::Time);
    e.adapt_node(node, PinId(1), &DataType::Color).unwrap();
    assert_eq!(stored(&e, node, "target"), "".into());
    assert_eq!(output(&e, node), DataType::Any);
}

#[test]
fn output_type_is_stable_on_failure_and_invalid_targets_yield_any() {
    let mut e = TickEngine::new();
    let node = add(
        &mut e,
        "convert",
        &[
            prop("value_type", "quantity.power"),
            prop("target", "signal_strength"),
        ],
    );
    e.set_input_value(node, PinId(1), Some(1i64.into()))
        .unwrap();
    assert_eq!(pull(&mut e, node), Some(30i64.into()));
    e.set_input_value(node, PinId(1), Some(0i64.into()))
        .unwrap();
    assert_eq!(
        pull(&mut e, node),
        None,
        "failed conversion must not keep the previous value"
    );
    assert_eq!(output(&e, node), q("signal_strength"));
    e.set_node_property(node, "target", "unknown".into())
        .unwrap();
    assert_eq!(stored(&e, node, "target"), "unknown".into());
    assert_eq!(output(&e, node), DataType::Any);
    assert_eq!(pull(&mut e, node), None);
}

#[test]
fn quantity_and_unit_selections_survive_serialized_graph_round_trip() {
    let mut e = TickEngine::new();
    // Restore in unit-first order to catch accidental unit resets while loading.
    let node = add(
        &mut e,
        "convert",
        &[
            prop("unit", "mired"),
            prop("target", "quantity:quantity.color_temperature"),
            prop("value_type", "number"),
        ],
    );
    e.set_input_value(node, PinId(1), Some(250i64.into()))
        .unwrap();
    assert_eq!(pull(&mut e, node), Some(4000i64.into()));
    assert_eq!(output(&e, node), q("color_temperature"));
    let graph = e.export_graph();
    let json = serde_json::to_string(&graph).unwrap();
    let mut restored = TickEngine::new();
    assert!(
        restored
            .load_graph(&serde_json::from_str(&json).unwrap())
            .is_empty()
    );
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(pull(&mut restored, node), Some(4000i64.into()));
    restored
        .set_node_property(node, "target", "quantity:quantity.power".into())
        .unwrap();
    assert_eq!(stored(&restored, node, "unit"), "watt".into());
    restored
        .set_node_property(node, "unit", "kilowatt".into())
        .unwrap();
    assert_eq!(pull(&mut restored, node), Some(250000i64.into()));
    restored
        .set_node_property(node, "unit", "invalid".into())
        .unwrap();
    assert_eq!(pull(&mut restored, node), None);
}

#[test]
fn adaptation_preserves_available_target_and_valid_unit() {
    let mut node = builtin::ConvertNode::new(NodeId::generate());
    assert!(!node.adapt_to(PinId(0), &DataType::Number));
    assert!(node.adapt_to(PinId(1), &DataType::Number));
    let first = convert::targets(&DataType::Number, &TypeRegistry::default())[0].clone();
    assert!(node.properties().contains(&prop("target", &first.id)));
    assert!(
        node.properties().contains(&prop(
            "unit",
            &quantity::kind(first.unit_of.as_deref().unwrap())
                .unwrap()
                .canonical
        ))
    );
    assert!(!node.adapt_to(PinId(1), &DataType::Number));
    node.set_property("target", "number".into());
    node.set_property("unit", "kelvin".into());
    assert!(node.adapt_to(PinId(1), &q("temperature")));
    assert!(node.properties().contains(&prop("unit", "kelvin")));
    assert!(node.adapt_to(PinId(1), &q("power")));
    assert!(node.properties().contains(&prop("target", "number")));
    assert!(node.properties().contains(&prop("unit", "watt")));
    assert_eq!(node.definition().cost, 1);
    assert!(node.is_pure());
    assert!(node.definition().adaptive);
}
