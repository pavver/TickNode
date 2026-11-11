use super::*;
use serde_json::json;

fn quantity_catalog() -> Value {
    json!({"number":{"set_value":{"fields":{
        "power":{"required":true,"selector":{"number":{"unit_of_measurement":"kW"}}},
        "temperature":{"selector":{"number":{"unit_of_measurement":"°F"}}},
        "unmapped":{"selector":{"number":{"unit_of_measurement":"widgets"}}}
    }}}})
}

#[test]
fn number_selector_units_are_snapshotted_and_converted_back_for_ha() {
    let catalog = services(&quantity_catalog());
    let fields = &catalog["number.set_value"].fields;
    assert_eq!(
        fields[0].data_type,
        DataType::Named("quantity.power".into())
    );
    assert_eq!(fields[0].unit.as_deref(), Some("kW"));
    assert_eq!(fields[0].json(&2000.0.into()), json!(2.0));
    assert!((fields[1].json(&20.0.into()).as_f64().unwrap() - 68.0).abs() < 1e-10);
    assert_eq!(fields[2].data_type, DataType::Number);
    assert_eq!(fields[2].unit, None);
    assert_eq!(fields[2].json(&2000i64.into()), json!(2000));

    crate::with_hub(|hub, _| hub.services = catalog);
    let nil = NodeId(ticknode_shared::Uuid::nil());
    let mut action = ActionNode::new(nil);
    action.set_property("action", "number.set_value".into());
    let properties = action.properties();
    let definition = action.definition();
    crate::with_hub(|hub, _| hub.services.clear());
    let mut restored = ActionNode::new(nil);
    for (name, value) in properties.clone() {
        restored.set_property(&name, value);
    }
    assert_eq!(restored.properties(), properties);
    assert_eq!(restored.definition(), definition);
    assert_eq!(restored.service.fields[0].json(&2000.0.into()), json!(2.0));
    assert_eq!(definition.pins[2].id, PinId(2));
    assert!(definition.pins[2].required);
}

#[test]
fn legacy_number_fields_without_unit_metadata_remain_unscaled() {
    let mut action = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    action.set_property("fields", super::node::strings(["power:number".into()]));
    assert_eq!(action.service.fields[0].data_type, DataType::Number);
    assert_eq!(action.service.fields[0].unit, None);
    assert_eq!(action.service.fields[0].json(&2000i64.into()), json!(2000));
}
