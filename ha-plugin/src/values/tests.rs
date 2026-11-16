use super::*;
use serde_json::json;
use ticknode_shared::{Color, plugin_values::project};
fn state(id: &str, text: &str, attributes: serde_json::Value) -> State {
    serde_json::from_value(
        json!({"entity_id": id, "state": text, "attributes": attributes,
        "last_changed": "2026-10-30T12:00:00Z", "last_updated": "2026-10-30T12:01:00Z",
        "context": {"id": "test"}}),
    )
    .unwrap()
}
fn ty(fields: &[ValueField], path: &str) -> DataType {
    fields
        .iter()
        .find(|f| f.path == path)
        .unwrap_or_else(|| panic!("missing {path}"))
        .data_type
        .clone()
}
#[test]
fn state_quantity_bool_datetime_and_duration_match_entity_conversion() {
    for state in [
        state(
            "sensor.power",
            "1.5",
            json!({"device_class":"power","unit_of_measurement":"kW"}),
        ),
        state("light.lamp", "on", json!({})),
        state(
            "sensor.alarm",
            "2026-10-30T15:00:00+02:00",
            json!({"device_class":"timestamp"}),
        ),
        state(
            "sensor.duration",
            "2",
            json!({"device_class":"duration","unit_of_measurement":"min"}),
        ),
    ] {
        let (value, fields) = tree(&state, None);
        let kind = crate::entity::infer_value_type(&state);
        assert_eq!(
            project(&value, "state"),
            crate::entity::typed_value(&state, kind).as_ref()
        );
        assert_eq!(ty(&fields, "state"), kind.data_type());
        assert_eq!(
            project(&value, "state_text"),
            Some(&DataValue::String(state.state.clone()))
        );
        assert!(matches!(
            project(&value, "last_changed"),
            Some(DataValue::DateTime(_))
        ));
    }
    let (value, _) = tree(
        &state(
            "sensor.power",
            "1.5",
            json!({"device_class":"power","unit_of_measurement":"kW"}),
        ),
        None,
    );
    assert_eq!(project(&value, "state"), Some(&DataValue::from(1500i64)));
}
#[test]
fn attributes_are_recursive_typed_and_catalog_matches_every_path() {
    let state = state(
        "climate.room",
        "heat",
        json!({"temperature_unit":"°F", "current_temperature":68,
        "temperature":77, "brightness":100, "enabled":true,
        "nested":{"when":"2026-10-30T12:00:00Z", "items":[{"temperature":86,"ok":false}]}}),
    );
    let (value, fields) = tree(&state, None);
    assert_eq!(
        ty(&fields, "attributes.current_temperature"),
        DataType::Named("quantity.temperature".into())
    );
    assert_eq!(
        project(&value, "attributes.current_temperature"),
        Some(&20.0.into())
    );
    assert_eq!(ty(&fields, "attributes.brightness"), DataType::Number);
    assert_eq!(ty(&fields, "attributes.enabled"), DataType::Bool);
    assert_eq!(ty(&fields, "attributes.nested.when"), DataType::DateTime);
    assert_eq!(
        ty(&fields, "attributes.nested.items"),
        DataType::List(Box::new(DataType::Object))
    );
    assert!(
        fields
            .iter()
            .all(|field| ticknode_shared::plugin_values::valid_path(&field.path))
    );
    assert_eq!(
        project(&value, "attributes.nested.items.0.temperature"),
        Some(&30.0.into())
    );
    for field in &fields {
        assert!(project(&value, &field.path).is_some(), "{}", field.path);
    }
}
#[test]
fn color_and_typed_client_attributes_do_not_remain_json() {
    let (value, fields) = tree(
        &state(
            "light.lamp",
            "on",
            json!({"rgb_color":[255,0,0],"hs_color":[120,100],"brightness":128}),
        ),
        None,
    );
    assert_eq!(
        project(&value, "attributes.rgb_color"),
        Some(&DataValue::Color(Color {
            r: 255,
            g: 0,
            b: 0,
            a: 255
        }))
    );
    assert_eq!(
        project(&value, "attributes.hs_color"),
        Some(&DataValue::Color(Color {
            r: 0,
            g: 255,
            b: 0,
            a: 255
        }))
    );
    assert_eq!(ty(&fields, "attributes.hs_color"), DataType::Color);
    let (value, fields) = tree(
        &state("update.router", "on", json!({"in_progress":45})),
        None,
    );
    assert_eq!(
        project(&value, "attributes.in_progress"),
        Some(&DataValue::Bool(true))
    );
    assert_eq!(ty(&fields, "attributes.in_progress"), DataType::Bool);
}

#[test]
fn climate_units_pending_keep_the_contract_without_exposing_uncalibrated_numbers() {
    let state = state("climate.room", "heat", json!({"current_temperature":68}));
    let (value, fields) = tree(&state, None);
    assert!(project(&value, "attributes.current_temperature").is_none());
    assert_eq!(
        ty(&fields, "attributes.current_temperature"),
        DataType::Named("quantity.temperature".into())
    );
    let (value, _) = tree(&state, Some("°F"));
    assert_eq!(
        project(&value, "attributes.current_temperature"),
        Some(&20.0.into())
    );
    let (value, fields) = tree(
        &super::tests::state("light.lamp", "on", json!({"color_temp":250})),
        None,
    );
    assert_eq!(
        project(&value, "attributes.color_temp"),
        Some(&4000i64.into())
    );
    assert_eq!(
        ty(&fields, "attributes.color_temp"),
        DataType::Named("quantity.color_temperature".into())
    );
}

#[test]
fn list_contracts_follow_compacted_null_elements() {
    let (value, fields) = tree(
        &state(
            "sensor.items",
            "ready",
            json!({"items":[null,{"when":"2026-10-30T12:00:00Z"}]}),
        ),
        None,
    );
    assert_eq!(
        ty(&fields, "attributes.items"),
        DataType::List(Box::new(DataType::Object))
    );
    assert!(
        fields
            .iter()
            .all(|field| ticknode_shared::plugin_values::valid_path(&field.path))
    );
    assert_eq!(
        project(&value, "attributes.items.0.when")
            .unwrap()
            .get_type(),
        DataType::DateTime
    );
    assert!(
        fields
            .iter()
            .all(|field| project(&value, &field.path).is_some()),
        "{fields:?}"
    );
}

#[test]
fn live_tree_retains_quantity_contracts_in_objects_and_lists_without_json_metadata() {
    let (value, _) = tree(
        &state(
            "climate.room",
            "heat",
            json!({"current_temperature":68, "nested":{"items":[{"temperature":86}]}}),
        ),
        Some("°F"),
    );
    let registry = ticknode_shared::TypeRegistry::from_definitions(
        ticknode_shared::quantity::builtin_quantities(),
    );
    for path in [
        "attributes.current_temperature",
        "attributes.nested.items.0.temperature",
    ] {
        assert_eq!(
            ticknode_shared::effective_type(Some(&value), &DataType::Object, path, &registry),
            Some(DataType::Named("quantity.temperature".into()))
        );
    }
    let json = crate::convert::value_to_json(&value);
    let text = serde_json::to_string(&json).unwrap();
    assert!(!text.contains("data_type"), "{text}");
    assert!(!text.contains("description"), "{text}");
    assert_eq!(json["attributes"]["current_temperature"], json!(20));
}
