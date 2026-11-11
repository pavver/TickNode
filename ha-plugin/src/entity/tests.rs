use super::*;
use crate::entities::test_state;
use chrono::{DateTime, Utc};
use serde_json::json;

fn data(outputs: &[Output], pin: u32) -> Option<&DataValue> {
    outputs.iter().find_map(|o| match o {
        Output::Data(p, v) if p.0 == pin => Some(v),
        _ => None,
    })
}

#[test]
fn infers_types_from_domains_attributes_and_state_text() {
    for (entity, text, attrs, expected) in [
        (
            "sensor.voltage",
            "unavailable",
            json!({"unit_of_measurement":"V"}),
            ValueType::Quantity("quantity.voltage"),
        ),
        (
            "sensor.energy",
            "unknown",
            json!({"state_class":"total"}),
            ValueType::Number,
        ),
        (
            "binary_sensor.door",
            "unavailable",
            json!({}),
            ValueType::Bool,
        ),
        ("input_select.mode", "eco", json!({}), ValueType::Text),
        (
            "sensor.alarm",
            "unavailable",
            json!({"device_class":"timestamp"}),
            ValueType::DateTime,
        ),
        (
            "input_datetime.alarm",
            "unknown",
            json!({"has_date":true,"has_time":true}),
            ValueType::DateTime,
        ),
        (
            "input_datetime.time",
            "07:30",
            json!({"has_date":false,"has_time":true}),
            ValueType::Text,
        ),
        ("sensor.n", "230.5", json!({}), ValueType::Number),
        ("sensor.n", "NaN", json!({}), ValueType::Text),
        ("sensor.n", "inf", json!({}), ValueType::Text),
        ("sensor.n", "unknown", json!({}), ValueType::Text),
        ("sensor.n", "active", json!({}), ValueType::Bool),
        (
            "sensor.n",
            "2026-10-30T10:00:00+02:00",
            json!({}),
            ValueType::DateTime,
        ),
    ] {
        assert_eq!(
            infer_value_type(&test_state(entity, text, attrs)),
            expected,
            "{entity}: {text}"
        );
    }
    for domain in ["input_number", "number", "counter"] {
        assert_eq!(
            infer_value_type(&test_state(&format!("{domain}.x"), "unknown", json!({}))),
            ValueType::Number
        );
    }
    for domain in [
        "switch",
        "light",
        "fan",
        "input_boolean",
        "lock",
        "cover",
        "person",
        "device_tracker",
        "siren",
        "automation",
        "script",
        "remote",
        "humidifier",
    ] {
        assert_eq!(
            infer_value_type(&test_state(&format!("{domain}.x"), "unknown", json!({}))),
            ValueType::Bool
        );
    }
}

#[test]
fn typed_values_are_finite_and_unavailable_values_clear_every_type() {
    let convert = |text, kind| typed_value(&test_state("sensor.x", text, json!({})), kind);
    assert_eq!(convert("230.5", ValueType::Number), Some(230.5.into()));
    assert_eq!(convert("hello", ValueType::Text), Some("hello".into()));
    for (yes, no) in [
        ("on", "off"),
        ("open", "closed"),
        ("home", "not_home"),
        ("true", "false"),
        ("playing", "paused"),
        ("locked", "unlocked"),
        ("detected", "clear"),
        ("active", "inactive"),
        ("occupied", "unoccupied"),
        ("connected", "disconnected"),
    ] {
        assert_eq!(convert(yes, ValueType::Bool), Some(true.into()));
        assert_eq!(convert(no, ValueType::Bool), Some(false.into()));
    }
    let expected = DateTime::parse_from_rfc3339("2026-10-30T08:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert_eq!(
        convert("2026-10-30T10:00:00+02:00", ValueType::DateTime),
        Some(DataValue::DateTime(expected))
    );
    for kind in [
        ValueType::Number,
        ValueType::Bool,
        ValueType::Text,
        ValueType::DateTime,
    ] {
        for text in ["unknown", "unavailable"] {
            assert_eq!(convert(text, kind), None);
        }
    }
    for text in ["NaN", "inf", "-inf", "hello"] {
        assert_eq!(convert(text, ValueType::Number), None);
    }
    assert_eq!(convert("On", ValueType::Bool), None);
    assert_eq!(convert("07:30", ValueType::DateTime), None);
}

#[test]
fn baselines_preserve_previous_and_changes_filter_only_the_exec_output() {
    let old = test_state("sensor.voltage", "230", json!({"unit_of_measurement":"V"}));
    let new = test_state(
        "sensor.voltage",
        "230",
        json!({"unit_of_measurement":"V", "custom":42}),
    );
    let baseline = outputs(Some(&new), ValueType::Number);
    assert_eq!(data(&baseline, 1), Some(&230.0.into()));
    assert_eq!(data(&baseline, 3), Some(&"V".into()));
    assert_eq!(
        field_number(data(&baseline, 4).unwrap(), "custom"),
        Some(42.0)
    );
    assert_eq!(
        data(&baseline, 5).unwrap().get_type(),
        DataType::Named(STATE_TYPE.into())
    );
    assert!(!baseline.iter().any(|o| matches!(
        o,
        Output::Exec(_) | Output::Clear(PinId(2)) | Output::Data(PinId(2), _)
    )));
    let mut change = StateChange {
        entity_id: "sensor.voltage".into(),
        old_state: Some(old),
        new_state: Some(new),
    };
    let state = change_outputs(&change, ValueType::Number, FireOn::State);
    let any = change_outputs(&change, ValueType::Number, FireOn::Any);
    assert!(!state.contains(&Output::Exec(PinId(0))));
    assert_eq!(any[..any.len() - 1], state);
    assert_eq!(any.last(), Some(&Output::Exec(PinId(0))));
    assert_eq!(data(&state, 2), Some(&230.0.into()));
    change.new_state.as_mut().unwrap().state = "231".into();
    assert!(
        change_outputs(&change, ValueType::Number, FireOn::State).contains(&Output::Exec(PinId(0)))
    );
    change.old_state = None;
    assert!(
        change_outputs(&change, ValueType::Number, FireOn::State)
            .contains(&Output::Clear(PinId(2)))
    );
    change.new_state = None;
    let removed = change_outputs(&change, ValueType::Number, FireOn::State);
    for pin in 1..=5 {
        assert!(removed.contains(&Output::Clear(PinId(pin))));
    }
    assert!(removed.contains(&Output::Exec(PinId(0))));
}

#[test]
fn property_order_preserves_manual_type_after_inference_on_load() {
    let node = NodeId(ticknode_shared::Uuid::nil());
    with_hub(|hub, _| {
        hub.live.snapshot(&[test_state(
            "sensor.voltage",
            "230",
            json!({"unit_of_measurement":"V"}),
        )])
    });
    let mut entity = Entity::new(node);
    entity.set_property("entity_id", " sensor.voltage ".into());
    assert_eq!(
        entity.definition().pins[1].pin_type,
        PinType::Data(DataType::Named("quantity.voltage".into()))
    );
    entity.set_property("value_type", "bool".into());
    entity.set_property("fire_on", "any".into());
    let saved = entity.properties();
    assert_eq!(
        saved.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["entity_id", "value_type", "fire_on"]
    );
    let mut restored = Entity::new(node);
    for (key, value) in saved.clone() {
        restored.set_property(&key, value);
    }
    assert_eq!(restored.properties(), saved);
    assert_eq!(
        restored.definition().pins[2].pin_type,
        PinType::Data(DataType::Bool)
    );
    restored.set_property("value_type", "invalid".into());
    restored.set_property("fire_on", "invalid".into());
    restored.set_property("entity_id", false.into());
    restored.set_property("unrelated", "ignored".into());
    assert_eq!(restored.properties(), saved);
    restored.set_property("entity_id", "sensor.missing".into());
    assert_eq!(restored.value_type, ValueType::Bool);
    with_hub(|hub, _| hub.live.reset());
    let mut offline = Entity::new(node);
    offline.set_property("entity_id", "sensor.voltage".into());
    assert_eq!(offline.value_type, ValueType::Text);
}
