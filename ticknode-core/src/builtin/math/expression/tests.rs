use super::*;
use crate::TickEngine;

#[test]
fn test_node_definition_cost_and_pure() {
    let node = MathExpressionNode::new(NodeId::generate());
    assert!(node.is_pure());
    let def = node.definition();
    assert_eq!(def.cost, 1);
    assert!(def.adaptive);
    assert_eq!(def.type_name, "math/expression");
    assert_eq!(def.title, "Formula");
}

#[test]
fn test_legacy_alphabetical_mapping_without_pin_ids() {
    let mut engine = TickEngine::new();
    let expr = NodeId::generate();
    let src = NodeId::generate();
    engine.create_node(src, "const", &[]).unwrap();
    // Legacy creation without pin_ids
    engine
        .create_node(
            expr,
            "math/expression",
            &[("expression".into(), DataValue::String("b + a".into()))],
        )
        .unwrap();

    let def = engine.node_definition(expr).unwrap();
    // In legacy alphabetical order: a -> 1, b -> 2
    let pin_a = def.pins.iter().find(|p| p.name == "a").unwrap();
    let pin_b = def.pins.iter().find(|p| p.name == "b").unwrap();
    assert_eq!(pin_a.id, PinId(1));
    assert_eq!(pin_b.id, PinId(2));

    // Connect to b (PinId 2)
    engine.connect_data(src, PinId(0), expr, PinId(2)).unwrap();
    assert_eq!(engine.data_connections().len(), 1);
}

#[test]
fn test_stable_pins_and_rename_in_place() {
    let mut node = MathExpressionNode::new(NodeId::generate());
    node.restored(); // interactive mode

    // Set expression "x + y"
    node.set_property("expression", "x + y".into());
    let x_pin = node.pins.iter().find(|(n, _, _)| n == "x").unwrap().1;
    let y_pin = node.pins.iter().find(|(n, _, _)| n == "y").unwrap().1;

    // Add a variable: "x + y + z" -> existing pins x and y stay stable
    node.set_property("expression", "x + y + z".into());
    assert_eq!(
        node.pins.iter().find(|(n, _, _)| n == "x").unwrap().1,
        x_pin
    );
    assert_eq!(
        node.pins.iter().find(|(n, _, _)| n == "y").unwrap().1,
        y_pin
    );
    let z_pin = node.pins.iter().find(|(n, _, _)| n == "z").unwrap().1;
    assert_ne!(z_pin, x_pin);
    assert_ne!(z_pin, y_pin);

    // Rename variable in place: "x + y_renamed + z" -> y_renamed inherits y's pin
    node.set_property("expression", "x + y_renamed + z".into());
    assert_eq!(
        node.pins.iter().find(|(n, _, _)| n == "x").unwrap().1,
        x_pin
    );
    assert_eq!(
        node.pins
            .iter()
            .find(|(n, _, _)| n == "y_renamed")
            .unwrap()
            .1,
        y_pin
    );
    assert_eq!(
        node.pins.iter().find(|(n, _, _)| n == "z").unwrap().1,
        z_pin
    );
}

#[test]
fn test_input_adaptation_and_output_inference() {
    let mut node = MathExpressionNode::new(NodeId::generate());
    node.restored();
    node.set_property("expression", "p * 2".into());

    let p_pin = node.pins.iter().find(|(n, _, _)| n == "p").unwrap().1;
    let power = DataType::Named("quantity.power".into());

    // Adapt p to quantity.power
    assert!(node.adapts_on_connect(p_pin));
    assert!(node.adapt_to(p_pin, &power));

    // Inferred output type follows quantity
    let def = node.definition();
    let res_pin = def.pins.iter().find(|p| p.name == "Result").unwrap();
    assert_eq!(res_pin.pin_type, PinType::Data(power.clone()));

    // When expression is comparison, output type is Bool
    node.set_property("expression", "p > 100".into());
    let def = node.definition();
    let res_pin = def.pins.iter().find(|p| p.name == "Result").unwrap();
    assert_eq!(res_pin.pin_type, PinType::Data(DataType::Bool));
}

#[test]
fn test_explicit_value_type_override() {
    let mut node = MathExpressionNode::new(NodeId::generate());
    node.restored();
    node.set_property("expression", "a * b".into());
    let power = DataType::Named("quantity.power".into());
    node.set_property("value_type", "quantity.power".into());

    let def = node.definition();
    let res_pin = def.pins.iter().find(|p| p.name == "Result").unwrap();
    assert_eq!(res_pin.pin_type, PinType::Data(power));
}

#[test]
fn test_runtime_error_reporting() {
    let mut engine = TickEngine::new();
    let expr = NodeId::generate();
    engine
        .create_node(
            expr,
            "math/expression",
            &[("expression".into(), DataValue::String("10 / 0".into()))],
        )
        .unwrap();

    let output = engine.pull_data(expr, PinId(0), &mut vec![]);
    // No output produced on division by zero
    assert_eq!(output, None);
    engine.take_node_error_changes();
    // Error was reported on the node
    assert!(engine.node_errors().iter().any(|e| e.node == expr));
}

#[test]
fn test_validation_rejects_syntax_errors() {
    let node = MathExpressionNode::new(NodeId::generate());
    assert!(
        node.validate_property("expression", &DataValue::String("1 + * 2".into()))
            .is_err()
    );
    assert!(
        node.validate_property("expression", &DataValue::String("other.x + 1".into()))
            .is_err()
    );
    assert!(
        node.validate_property("expression", &DataValue::String("var.x + 1".into()))
            .is_ok()
    );
    assert!(
        node.validate_property("expression", &DataValue::String("a + b".into()))
            .is_ok()
    );
}

#[test]
fn test_persistence_roundtrip() {
    let mut engine = TickEngine::new();
    let expr = NodeId::generate();
    let power = DataType::Named("quantity.power".into());
    engine
        .create_node(
            expr,
            "math/expression",
            &[("expression".into(), DataValue::String("x * 2".into()))],
        )
        .unwrap();

    let x_pin = engine
        .node_definition(expr)
        .unwrap()
        .pins
        .iter()
        .find(|p| p.name == "x")
        .unwrap()
        .id;
    assert!(engine.adapt_node(expr, x_pin, &power).is_ok());

    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    let warnings = restored.load_graph(&graph);
    assert!(warnings.is_empty(), "{:?}", warnings);
    assert_eq!(restored.export_graph(), graph);

    let def = restored.node_definition(expr).unwrap();
    let restored_x = def.pins.iter().find(|p| p.name == "x").unwrap();
    assert_eq!(restored_x.pin_type, PinType::Data(power));
}
