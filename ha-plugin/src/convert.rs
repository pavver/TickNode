//! Conversions between Home Assistant JSON / `ha-core-client` types and TickNode values.
//!
//! Time zones: `DataValue::DateTime` is a `DateTime<Utc>`, and Home Assistant timestamps are parsed
//! by `ha-core-client` into `DateTime<Utc>` as well (the offset in the text is applied), so they pass
//! through unchanged; no conversion happens here.

use std::collections::BTreeMap;

use ha_core_client::State;
use serde_json::{Map, Number, Value};
use ticknode_plugin_sdk::{DataType, DataValue, NumberValue, ObjectField, StructValue};
use ticknode_shared::text;

/// Name of the plugin-defined struct type for entity states.
pub const STATE_TYPE: &str = "ha.state";

fn object(fields: BTreeMap<String, DataValue>) -> DataValue {
    DataValue::Object {
        fields: fields.into_iter().map(|(name, value)| (name.clone(), ObjectField { name, description: String::new(), value })).collect(),
        type_name: None,
    }
}

/// JSON to a value. `null` has no counterpart: it becomes `None` and the key or element is skipped.
pub fn json_to_value(json: &Value) -> Option<DataValue> {
    Some(match json {
        Value::Null => return None,
        Value::Bool(b) => DataValue::Bool(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => DataValue::Number(NumberValue::Int(i)),
            None => DataValue::from(n.as_f64()?),
        },
        Value::String(s) => DataValue::String(s.clone()),
        Value::Array(items) => {
            let elements: Vec<DataValue> = items.iter().filter_map(json_to_value).collect();
            let mut types = elements.iter().map(DataValue::get_type);
            let element_type = match types.next() {
                Some(first) if types.all(|t| t == first) => first,
                _ => DataType::Any,
            };
            DataValue::List { element_type, elements }
        }
        Value::Object(map) => object(map.iter().filter_map(|(k, v)| Some((k.clone(), json_to_value(v)?))).collect()),
    })
}

/// A value to JSON for a service call. Times and colors become their text form, durations seconds.
pub fn value_to_json(value: &DataValue) -> Value {
    match value {
        DataValue::Bool(b) => Value::Bool(*b),
        DataValue::Number(NumberValue::Int(i)) => Value::from(*i),
        DataValue::Number(NumberValue::Float(f)) => Number::from_f64(*f).map_or(Value::Null, Value::Number),
        DataValue::String(s) => Value::String(s.clone()),
        DataValue::Time(t) => Value::String(t.format("%H:%M:%S").to_string()),
        DataValue::DateTime(dt) => Value::String(dt.to_rfc3339()),
        DataValue::Duration(d) => Number::from_f64(d.as_secs_f64()).map_or(Value::Null, Value::Number),
        DataValue::Color(_) => Value::String(text::format_value(value)),
        DataValue::List { elements, .. } => Value::Array(elements.iter().map(value_to_json).collect()),
        DataValue::Object { fields, .. } => Value::Object(fields.iter().map(|(k, f)| (k.clone(), value_to_json(&f.value))).collect::<Map<_, _>>()),
    }
}

/// Attributes of a state as an untyped object. `AttributeSet` serializes externally tagged
/// (`{"Sensor": {...}}`), so the tag is peeled off; absent (`null`) attributes are left out.
fn attributes_value(state: &State) -> DataValue {
    let json = serde_json::to_value(&state.attributes).unwrap_or(Value::Null);
    let inner = match json {
        Value::Object(map) if map.len() == 1 => map.into_iter().next().map_or(Value::Null, |(_, v)| v),
        other => other,
    };
    match json_to_value(&inner) {
        Some(value @ DataValue::Object { .. }) => value,
        _ => object(BTreeMap::new()),
    }
}

/// An `ha.state` value of a Home Assistant state.
pub fn state_value(state: &State) -> DataValue {
    StructValue::new(STATE_TYPE)
        .field("entity_id", state.entity_id.as_str())
        .field("state", state.state.as_str())
        .field("attributes", attributes_value(state))
        .field("last_changed", DataValue::DateTime(state.last_changed))
        .field("last_updated", DataValue::DateTime(state.last_updated))
        .build()
}

/// The `ha.state` standing for "no state": the entity did not exist (before it was created, after it was
/// removed) or the read failed. The host cannot clear an output pin, so a value is always sent instead of
/// leaving the previous one in place. `state` is empty (Home Assistant itself never reports an empty state).
pub fn missing_state(entity_id: &str) -> DataValue {
    StructValue::new(STATE_TYPE)
        .field("entity_id", entity_id)
        .field("state", "")
        .field("attributes", object(BTreeMap::new()))
        .field("last_changed", DataValue::DateTime(Default::default()))
        .field("last_updated", DataValue::DateTime(Default::default()))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    fn field<'a>(value: &'a DataValue, name: &str) -> &'a DataValue {
        ticknode_plugin_sdk::field(value, name).unwrap_or_else(|| panic!("no field {name}"))
    }

    fn state(entity_id: &str, state: &str, attributes: Value, changed: &str) -> State {
        serde_json::from_value(json!({
            "entity_id": entity_id, "state": state, "attributes": attributes,
            "last_changed": changed, "last_updated": changed,
            "context": {"id": "01X", "parent_id": null, "user_id": null}
        })).unwrap()
    }

    #[test]
    fn state_converts_with_utc_timestamps() {
        // +02:00 in the text: the instant is 2026-10-30 21:15:42.5 UTC
        let s = state("sensor.temp", "21.5", json!({"unit_of_measurement": "°C", "friendly_name": "Temp", "tags": ["a", "b"], "extra": {"n": 1, "gone": null}}), "2026-10-30T23:15:42.500000+02:00");
        let v = state_value(&s);
        assert_eq!(v.get_type(), DataType::Named(STATE_TYPE.into()));
        assert_eq!(field(&v, "entity_id"), &DataValue::String("sensor.temp".into()));
        assert_eq!(field(&v, "state"), &DataValue::String("21.5".into()));
        let expected = Utc.with_ymd_and_hms(2026, 10, 30, 21, 15, 42).unwrap() + chrono::Duration::milliseconds(500);
        assert_eq!(field(&v, "last_changed"), &DataValue::DateTime(expected));
        assert_eq!(field(&v, "last_updated"), &DataValue::DateTime(expected));
        let attrs = field(&v, "attributes");
        assert_eq!(attrs.get_type(), DataType::Object);
        assert_eq!(field(attrs, "unit_of_measurement"), &DataValue::String("°C".into()));
        assert_eq!(field(attrs, "friendly_name"), &DataValue::String("Temp".into()));
        let DataValue::List { element_type, elements } = field(attrs, "tags") else { panic!("tags") };
        assert_eq!((element_type, elements.len()), (&DataType::String, 2));
        // Nested objects keep their keys, nulls are skipped, absent typed attributes do not appear
        assert_eq!(field(field(attrs, "extra"), "n"), &DataValue::Number(NumberValue::Int(1)));
        assert!(ticknode_plugin_sdk::field(field(attrs, "extra"), "gone").is_none());
        assert!(ticknode_plugin_sdk::field(attrs, "device_class").is_none());
    }

    #[test]
    fn typed_attributes_of_other_domains_are_flat() {
        let s = state("climate.living", "heat", json!({"temperature": 21.0, "current_temperature": 19.5, "hvac_modes": ["heat", "off"], "custom": true}), "2026-10-30T10:00:00+00:00");
        let attrs = state_value(&s);
        let attrs = field(&attrs, "attributes");
        assert_eq!(field(attrs, "temperature"), &DataValue::Number(NumberValue::Int(21)));
        assert_eq!(field(attrs, "current_temperature"), &DataValue::Number(NumberValue::Float(19.5)));
        assert_eq!(field(attrs, "custom"), &DataValue::Bool(true));
        assert!(matches!(field(attrs, "hvac_modes"), DataValue::List { .. }));
    }

    #[test]
    fn json_lists_get_an_element_type() {
        let list = json_to_value(&json!([1, "a", null])).unwrap();
        let DataValue::List { element_type, elements } = list else { panic!() };
        assert_eq!((element_type, elements.len()), (DataType::Any, 2));
        let DataValue::List { element_type, .. } = json_to_value(&json!([])).unwrap() else { panic!() };
        assert_eq!(element_type, DataType::Any);
        assert_eq!(json_to_value(&json!(2.5)), Some(DataValue::Number(NumberValue::Float(2.5))));
        assert_eq!(json_to_value(&Value::Null), None);
    }

    #[test]
    fn values_convert_to_json() {
        let mut fields = BTreeMap::new();
        fields.insert("brightness".to_string(), DataValue::from(120i64));
        fields.insert("rgb".to_string(), DataValue::List { element_type: DataType::Number, elements: vec![DataValue::from(1i64), DataValue::from(2.5)] });
        fields.insert("name".to_string(), DataValue::from("x"));
        fields.insert("t".to_string(), DataValue::Duration(std::time::Duration::from_millis(1500)));
        assert_eq!(value_to_json(&object(fields)), json!({"brightness": 120, "rgb": [1, 2.5], "name": "x", "t": 1.5}));
    }

    #[test]
    fn missing_state_is_an_empty_ha_state() {
        let v = missing_state("light.x");
        assert_eq!(v.get_type(), DataType::Named(STATE_TYPE.into()));
        assert_eq!(field(&v, "state"), &DataValue::String(String::new()));
        assert_eq!(field(&v, "entity_id"), &DataValue::String("light.x".into()));
    }
}
