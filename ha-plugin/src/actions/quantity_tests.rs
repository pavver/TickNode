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
    assert_eq!(fields[0].json(&2000.0.into()).unwrap(), json!(2.0));
    assert!((fields[1].json(&20.0.into()).unwrap().as_f64().unwrap() - 68.0).abs() < 1e-10);
    assert_eq!(fields[2].data_type, DataType::Number);
    assert_eq!(fields[2].unit, None);
    assert_eq!(fields[2].json(&2000i64.into()).unwrap(), json!(2000));

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
    assert_eq!(
        restored.service.fields[0].json(&2000.0.into()).unwrap(),
        json!(2.0)
    );
    assert_eq!(definition.pins[2].id, PinId(2));
    assert!(definition.pins[2].required);
}

#[test]
fn legacy_number_fields_without_unit_metadata_remain_unscaled() {
    let mut action = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    action.set_property("fields", super::node::strings(["power:number".into()]));
    assert_eq!(action.service.fields[0].data_type, DataType::Number);
    assert_eq!(action.service.fields[0].unit, None);
    assert_eq!(
        action.service.fields[0].json(&2000i64.into()).unwrap(),
        json!(2000)
    );
}

#[test]
fn numeric_duration_selectors_keep_number_pins_and_raw_values() {
    for unit in ["d", "h", "min", "s", "ms", "µs", "μs", "us"] {
        let catalog = services(&json!({"number":{"set_value":{"fields":{
            "duration":{"selector":{"number":{"unit_of_measurement":unit}}}
        }}}}));
        let field = &catalog["number.set_value"].fields[0];
        assert_eq!(field.data_type, DataType::Number, "{unit}");
        assert_eq!(field.unit, None);
        assert_eq!(field.json(&2.5.into()).unwrap(), json!(2.5));
    }
}

#[test]
fn color_temperature_selectors_snapshot_kelvin_and_default_mired() {
    let catalog = services(&json!({"light":{"turn_on":{"fields":{
        "default":{"selector":{"color_temp":{"min":153,"max":500}}},
        "kelvin":{"required":true,"selector":{"color_temp":{"unit":"kelvin","min":2000,"max":6500}}},
        "mired":{"selector":{"color_temp":{"unit":"mired"}}}
    }}}}));
    for (field, unit, expected) in catalog["light.turn_on"]
        .fields
        .iter()
        .zip(["mired", "kelvin", "mired"])
        .zip([250.0, 4000.0, 250.0])
        .map(|((f, u), e)| (f, u, e))
    {
        assert_eq!(
            field.data_type,
            DataType::Named("quantity.color_temperature".into())
        );
        assert_eq!(field.unit.as_deref(), Some(unit));
        assert_eq!(field.json(&4000.0.into()), Some(json!(expected)));
    }
    crate::with_hub(|hub, _| hub.services = catalog);
    let nil = NodeId(ticknode_shared::Uuid::nil());
    let mut action = ActionNode::new(nil);
    action.set_property("action", "light.turn_on".into());
    let properties = action.properties();
    assert!(properties.iter().any(|(name, _)| name == "field_units"));
    let definition = action.definition();
    crate::with_hub(|hub, _| hub.services.clear());
    let mut restored = ActionNode::new(nil);
    for (name, value) in properties.clone() {
        restored.set_property(&name, value);
    }
    assert_eq!(restored.properties(), properties);
    assert_eq!(restored.definition(), definition);
    assert_eq!(
        restored.service.fields[0].json(&4000.0.into()),
        Some(json!(250.0))
    );
    assert_eq!(
        restored.service.fields[1].json(&4000.0.into()),
        Some(json!(4000.0))
    );
    assert!(restored.service.fields[0].json(&0.0.into()).is_none());
    let inputs = [(PinId(2), 0.0.into())].into_iter().collect();
    assert!(
        restored
            .execute(Trigger::Pin(PinId(0)), &inputs)
            .contains(&Output::Exec(FAILED))
    );
}

#[test]
fn number_selectors_convert_ambiguous_units_using_the_snapshotted_quantity() {
    for (class, symbol, canonical, expected) in [
        ("temperature", "K", 26.85, 300.0),
        ("power", "W", 1000.0, 1000.0),
        ("signal_strength", "W", 30.0, 1.0),
        ("signal_strength", "mW", -20.0, 0.01),
        ("energy_distance", "km/kWh", 20.0, 5.0),
    ] {
        let catalog = services(&json!({"number":{"set_value":{"fields":{
            "value":{"selector":{"number":{"device_class":class,"unit_of_measurement":symbol}}}
        }}}}));
        let field = &catalog["number.set_value"].fields[0];
        let converted = field.json(&canonical.into()).unwrap().as_f64().unwrap();
        assert!((converted - expected).abs() < 1e-10, "{class}/{symbol}");
        assert!(field.json(&f64::INFINITY.into()).is_none());
        if class == "signal_strength" {
            assert!(field.json(&4000.0.into()).is_none());
        }
    }
}
