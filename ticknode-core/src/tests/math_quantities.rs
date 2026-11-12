//! Built-in math nodes adapt to physical quantities on connection.
use crate::*;
use ticknode_shared::*;

fn create(
    e: &mut TickEngine,
    name: &str,
    props: &[(String, DataValue)],
) -> Result<NodeId, EngineError> {
    let id = NodeId::generate();
    e.create_node(id, name, props).map(|_| id)
}
fn power() -> DataType {
    DataType::Named("quantity.power".into())
}

fn data_type(engine: &TickEngine, node: NodeId, pin: u32, dir: PinDirection) -> DataType {
    let def = engine.node_definition(node).unwrap();
    let pin = def
        .pins
        .iter()
        .find(|p| p.id == PinId(pin) && p.direction == dir)
        .unwrap();
    match &pin.pin_type {
        PinType::Data(t) => t.clone(),
        PinType::Exec => panic!("data pin expected"),
    }
}

#[test]
fn add_keeps_the_connected_quantity_and_mul_scales_it() {
    let mut engine = TickEngine::new();
    let source = create(
        &mut engine,
        "const",
        &[
            ("value".into(), 1500i64.into()),
            ("value_type".into(), "quantity.power".into()),
        ],
    )
    .unwrap();
    let add = create(&mut engine, "math/add", &[]).unwrap();
    engine
        .connect_data(source, PinId(0), add, PinId(0))
        .unwrap();
    for (pin, dir) in [
        (0, PinDirection::Input),
        (1, PinDirection::Input),
        (2, PinDirection::Output),
    ] {
        assert_eq!(data_type(&engine, add, pin, dir), power());
    }
    assert_eq!(
        engine
            .export_graph()
            .nodes
            .iter()
            .find(|n| n.id == add)
            .unwrap()
            .properties,
        vec![("value_type".to_string(), DataValue::from("quantity.power"))]
    );

    let mul = create(&mut engine, "math/mul", &[]).unwrap();
    engine.connect_data(add, PinId(2), mul, PinId(0)).unwrap();
    assert_eq!(
        data_type(&engine, mul, 1, PinDirection::Input),
        DataType::Number
    );
    assert_eq!(data_type(&engine, mul, 2, PinDirection::Output), power());
}

#[test]
fn math_never_adapts_to_text_or_over_an_existing_wire() {
    let mut engine = TickEngine::new();
    let text = create(&mut engine, "const", &[("value".into(), "x".into())]).unwrap();
    let add = create(&mut engine, "math/add", &[]).unwrap();
    // Text converts into the Number contract (a dashed wire); the node keeps numbers.
    engine.connect_data(text, PinId(0), add, PinId(0)).unwrap();
    assert_eq!(
        data_type(&engine, add, 2, PinDirection::Output),
        DataType::Number
    );
    engine.disconnect_data(add, PinId(0));

    let number = create(&mut engine, "const", &[("value".into(), 2i64.into())]).unwrap();
    let watts = create(
        &mut engine,
        "const",
        &[
            ("value".into(), 5i64.into()),
            ("value_type".into(), "quantity.power".into()),
        ],
    )
    .unwrap();
    engine
        .connect_data(number, PinId(0), add, PinId(0))
        .unwrap();
    // B converts the quantity into the existing Number contract instead of retyping A's wire.
    engine.connect_data(watts, PinId(0), add, PinId(1)).unwrap();
    assert_eq!(
        data_type(&engine, add, 0, PinDirection::Input),
        DataType::Number
    );
    assert_eq!(engine.export_graph().data_connections.len(), 2);
}

#[test]
fn compare_adapts_both_inputs_to_the_quantity() {
    let mut engine = TickEngine::new();
    let temp = create(
        &mut engine,
        "const",
        &[
            ("value".into(), 19i64.into()),
            ("value_type".into(), "quantity.temperature".into()),
        ],
    )
    .unwrap();
    let cmp = create(&mut engine, "compare", &[]).unwrap();
    engine.connect_data(temp, PinId(0), cmp, PinId(0)).unwrap();
    let t = DataType::Named("quantity.temperature".into());
    assert_eq!(data_type(&engine, cmp, 1, PinDirection::Input), t);
    // 68 °F is stored canonically as 20 °C.
    engine
        .set_input_value(cmp, PinId(1), Some(DataValue::from(20i64)))
        .unwrap();
    assert_eq!(
        engine.pull_data(cmp, PinId(4), &mut Vec::new()),
        Some(DataValue::Bool(true))
    );
}
