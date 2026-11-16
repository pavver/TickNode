use super::*;
use std::sync::Arc;

#[test]
fn creating_and_editing_formulas_share_large_catalog_and_resolve_only_references() {
    let catalog = crate::plugin_values::tests::large_catalog();
    let mut engine = crate::TickEngine::new();
    engine
        .plugin_values
        .catalogs
        .insert("ha".into(), catalog.clone());
    let initial = Arc::strong_count(&catalog);
    let id = NodeId::generate();
    engine
        .create_node(
            id,
            "math/expression",
            &[(
                "expression".into(),
                "ha.sensor.entity_19999.attributes.field_29 * 2".into(),
            )],
        )
        .unwrap();
    assert_eq!(Arc::strong_count(&catalog), initial + 1);
    assert_eq!(
        engine.node_definition(id).unwrap().pins[0].pin_type,
        PinType::Data(DataType::Named("quantity.power".into()))
    );
    let mut node = MathExpressionNode::new(NodeId::generate());
    node.values_catalogs_changed(&engine.plugin_values.catalogs());
    assert!(Arc::ptr_eq(&node.catalogs["ha"], &catalog));
    node.set_property(
        "expression",
        "ha.sensor.entity_19999.attributes.field_29 * 2".into(),
    );
    node.restored();
    assert_eq!(node.referenced_types().len(), 1);
    node.set_property(
        "expression",
        "ha.sensor.entity_0.attributes.field_0 + ha.sensor.entity_1.attributes.field_1".into(),
    );
    assert_eq!(node.referenced_types().len(), 2);
    node.set_property("expression", "a + 1".into());
    assert!(node.referenced_types().is_empty());
    engine.remove_node(id);
    drop(node);
    assert_eq!(Arc::strong_count(&catalog), initial);
}
