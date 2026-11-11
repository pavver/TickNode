use super::*;
use crate::entities::test_state;
use serde_json::json;
use std::time::Duration;

#[test]
fn durations_infer_from_class_or_numeric_states_and_convert_each_unit() {
    for (unit, seconds) in [
        ("d", 86400.0),
        ("h", 3600.0),
        ("min", 60.0),
        ("s", 1.0),
        ("ms", 0.001),
        ("µs", 0.000001),
        ("μs", 0.000001),
        ("us", 0.000001),
    ] {
        for class in [json!("duration"), json!(null)] {
            let state = test_state(
                "sensor.elapsed",
                "2.5",
                json!({"device_class":class, "unit_of_measurement":unit}),
            );
            assert_eq!(infer_value_type(&state), ValueType::Duration, "{unit}");
            assert_eq!(
                typed_value(&state, ValueType::Duration),
                Some(DataValue::Duration(Duration::from_secs_f64(2.5 * seconds)))
            );
        }
        let state = test_state(
            "sensor.elapsed",
            "unavailable",
            json!({"device_class":"duration", "unit_of_measurement":unit}),
        );
        assert_eq!(infer_value_type(&state), ValueType::Duration);
        assert_eq!(typed_value(&state, ValueType::Duration), None);
    }
}

#[test]
fn duration_conversion_rejects_invalid_values_units_and_overflow() {
    for text in [
        "-1",
        "NaN",
        "inf",
        "-inf",
        "1e309",
        "1e308",
        "unknown",
        "unavailable",
        "hello",
    ] {
        let state = test_state("sensor.elapsed", text, json!({"unit_of_measurement":"d"}));
        assert_eq!(typed_value(&state, ValueType::Duration), None, "{text}");
    }
    for unit in [json!(null), json!(""), json!("widgets"), json!("m")] {
        let state = test_state("sensor.elapsed", "2", json!({"unit_of_measurement":unit}));
        assert_eq!(typed_value(&state, ValueType::Duration), None);
        assert_ne!(infer_value_type(&state), ValueType::Duration);
    }
    let state = test_state("sensor.elapsed", "0", json!({"unit_of_measurement":"s"}));
    assert_eq!(
        typed_value(&state, ValueType::Duration),
        Some(DataValue::Duration(Duration::ZERO))
    );
}

#[test]
fn duration_selection_persists_and_previous_values_use_their_own_units() {
    assert_eq!(ValueType::parse("duration"), Some(ValueType::Duration));
    assert_eq!(ValueType::Duration.as_str(), "duration");
    assert_eq!(ValueType::Duration.data_type(), DataType::Duration);
    let mut entity = Entity::new(NodeId(ticknode_shared::Uuid::nil()));
    entity.set_property("value_type", "duration".into());
    let properties = entity.properties();
    let mut restored = Entity::new(NodeId(ticknode_shared::Uuid::nil()));
    for (name, value) in properties.clone() {
        restored.set_property(&name, value);
    }
    assert_eq!(restored.properties(), properties);
    assert_eq!(
        restored.definition().pins[1].pin_type,
        PinType::Data(DataType::Duration)
    );
    assert_eq!(
        restored.definition().pins[2].pin_type,
        PinType::Data(DataType::Duration)
    );
    let change = StateChange {
        entity_id: "sensor.elapsed".into(),
        old_state: Some(test_state(
            "sensor.elapsed",
            "30",
            json!({"unit_of_measurement":"s"}),
        )),
        new_state: Some(test_state(
            "sensor.elapsed",
            "2",
            json!({"unit_of_measurement":"min"}),
        )),
    };
    let outputs = change_outputs(&change, ValueType::Duration, FireOn::State);
    assert!(outputs.contains(&Output::Data(
        PinId(1),
        DataValue::Duration(Duration::from_secs(120))
    )));
    assert!(outputs.contains(&Output::Data(
        PinId(2),
        DataValue::Duration(Duration::from_secs(30))
    )));
    let ticknode_plugin_sdk::PropertyKind::Select(options) = value::type_property().kind else {
        panic!("expected select options")
    };
    assert_eq!(options[3].0, "datetime");
    assert_eq!(options[4], ("duration".into(), "Duration".into()));
    assert_eq!(options[5].0, "quantity.power");
}
