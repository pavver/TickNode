use super::*;
use crate::entities::test_state;
use serde_json::json;

#[test]
fn inference_and_property_options_cover_quantities_and_unknown_units() {
    for (unit, class, expected) in [
        ("V", "voltage", ValueType::Quantity("quantity.voltage")),
        ("kW", "power", ValueType::Quantity("quantity.power")),
        ("%", "battery", ValueType::Quantity("quantity.percentage")),
        ("widgets", "", ValueType::Number),
    ] {
        let state = test_state(
            "sensor.x",
            "2",
            json!({"unit_of_measurement":unit, "device_class":class}),
        );
        assert_eq!(infer_value_type(&state), expected);
    }
    let mut entity = Entity::new(NodeId(ticknode_shared::Uuid::nil()));
    for definition in quantity::catalog() {
        entity.set_property("value_type", definition.name.clone().into());
        let expected = PinType::Data(DataType::Named(definition.name.clone()));
        assert_eq!(entity.definition().pins[1].pin_type, expected);
        assert_eq!(entity.definition().pins[2].pin_type, expected);
        let properties = entity.properties();
        let mut restored = Entity::new(NodeId(ticknode_shared::Uuid::nil()));
        for (name, value) in properties.clone() {
            restored.set_property(&name, value);
        }
        assert_eq!(restored.properties(), properties);
    }
}

#[test]
fn each_sample_uses_its_own_unit_and_mismatches_clear_only_the_affected_pin() {
    let power = ValueType::Quantity("quantity.power");
    let state = |text, unit| test_state("sensor.power", text, json!({"unit_of_measurement":unit}));
    let mut change = StateChange {
        entity_id: "sensor.power".into(),
        old_state: Some(state("500", "W")),
        new_state: Some(state("2", "kW")),
    };
    let outputs = change_outputs(&change, power, FireOn::State);
    assert!(outputs.contains(&Output::Data(PinId(1), 2000.0.into())));
    assert!(outputs.contains(&Output::Data(PinId(2), 500.0.into())));
    assert!(outputs.contains(&Output::Data(PinId(3), "kW".into())));
    assert_eq!(
        field_string(
            outputs
                .iter()
                .find_map(|o| match o {
                    Output::Data(PinId(5), v) => Some(v),
                    _ => None,
                })
                .unwrap(),
            "state"
        ),
        Some("2")
    );
    change.new_state = Some(state("3", "V"));
    let outputs = change_outputs(&change, power, FireOn::State);
    assert!(outputs.contains(&Output::Clear(PinId(1))));
    assert!(outputs.contains(&Output::Data(PinId(2), 500.0.into())));
    for unit in [json!(null), json!(""), json!("V")] {
        let state = test_state("sensor.power", "2", json!({"unit_of_measurement":unit}));
        assert_eq!(typed_value(&state, power), None);
        assert!(unit_mismatch(&state, power));
    }
    let state = test_state("sensor.temp", "68", json!({"unit_of_measurement":"°F"}));
    let temp = typed_value(&state, ValueType::Quantity("quantity.temperature")).unwrap();
    let DataValue::Number(temp) = temp else {
        panic!("expected a canonical number")
    };
    assert!((temp.to_f64() - 20.0).abs() < 1e-10);
    for text in ["NaN", "inf", "1e308", "unknown", "unavailable"] {
        assert_eq!(typed_value(&state_with_text(text), power), None);
    }
}

fn state_with_text(text: &str) -> ha_core_client::State {
    test_state("sensor.power", text, json!({"unit_of_measurement":"kW"}))
}
