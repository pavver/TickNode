use super::*;

#[test]
fn variable_properties_validate_before_replacing_existing_input_values() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "math/add", &[]).unwrap();
    let property = input_variable_property(PinId(0));
    engine
        .set_input_value(id, PinId(0), Some(42i64.into()))
        .unwrap();
    for value in ["not a reference".into(), 3i64.into()] {
        assert!(engine.set_node_property(id, &property, value).is_err());
        assert!(
            engine
                .node_model(id)
                .unwrap()
                .properties
                .contains(&(input_value_property(PinId(0)), 42i64.into()))
        );
    }
    engine
        .set_node_property(id, &property, "var.amount".into())
        .unwrap();
    let model = engine.node_model(id).unwrap();
    assert!(model.properties.contains(&(property, "var.amount".into())));
    assert!(
        !model
            .properties
            .iter()
            .any(|(name, _)| name == &input_value_property(PinId(0)))
    );
    assert_eq!(
        engine.input_variable(id, PinId(0)),
        Some(&VariableRef::auto("amount"))
    );
}
