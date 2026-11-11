//! Pure entity value type inference and conversion.

use chrono::{DateTime, Utc};
use ha_core_client::State;
use ticknode_plugin_sdk::{DataType, DataValue, quantity};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValueType {
    Number,
    Bool,
    #[default]
    Text,
    DateTime,
    Quantity(&'static str),
}

impl ValueType {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "number" => Self::Number,
            "bool" => Self::Bool,
            "text" => Self::Text,
            "datetime" => Self::DateTime,
            _ => Self::Quantity(
                quantity::catalog()
                    .iter()
                    .find(|d| d.name == text)?
                    .name
                    .as_str(),
            ),
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Bool => "bool",
            Self::Text => "text",
            Self::DateTime => "datetime",
            Self::Quantity(name) => name,
        }
    }

    pub fn data_type(self) -> DataType {
        match self {
            Self::Number => DataType::Number,
            Self::Bool => DataType::Bool,
            Self::Text => DataType::String,
            Self::DateTime => DataType::DateTime,
            Self::Quantity(name) => DataType::Named(name.into()),
        }
    }
}

fn on_off(text: &str) -> Option<bool> {
    match text {
        "on" | "open" | "home" | "true" | "playing" | "locked" | "detected" | "active"
        | "occupied" | "connected" => Some(true),
        "off" | "closed" | "not_home" | "false" | "paused" | "unlocked" | "clear" | "inactive"
        | "unoccupied" | "disconnected" => Some(false),
        _ => None,
    }
}

fn finite_number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|n| n.is_finite())
}

pub fn infer_value_type(state: &State) -> ValueType {
    let domain = state
        .entity_id
        .as_str()
        .split('.')
        .next()
        .unwrap_or_default();
    let attrs = crate::convert::attributes_json(state);
    if let Some((name, _)) = crate::units::state_unit(state) {
        return ValueType::Quantity(name);
    }
    if matches!(domain, "input_number" | "number" | "counter")
        || attrs
            .get("unit_of_measurement")
            .is_some_and(|v| !v.is_null())
        || attrs.get("state_class").is_some_and(|v| !v.is_null())
        || finite_number(&state.state).is_some()
    {
        ValueType::Number
    } else if matches!(
        domain,
        "binary_sensor"
            | "switch"
            | "light"
            | "fan"
            | "input_boolean"
            | "lock"
            | "cover"
            | "person"
            | "device_tracker"
            | "siren"
            | "automation"
            | "script"
            | "remote"
            | "humidifier"
    ) || on_off(&state.state).is_some()
    {
        ValueType::Bool
    } else if attrs["device_class"] == "timestamp"
        || (domain == "input_datetime" && attrs["has_date"] == true && attrs["has_time"] == true)
        || DateTime::parse_from_rfc3339(&state.state).is_ok()
    {
        ValueType::DateTime
    } else {
        ValueType::Text
    }
}

pub fn typed_value(state: &State, value_type: ValueType) -> Option<DataValue> {
    let text = state.state.as_str();
    if matches!(text, "unknown" | "unavailable") {
        return None;
    }
    match value_type {
        ValueType::Number => finite_number(text).map(DataValue::from),
        ValueType::Bool => on_off(text).map(DataValue::Bool),
        ValueType::Text => Some(text.into()),
        ValueType::DateTime => DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|dt| DataValue::DateTime(dt.with_timezone(&Utc))),
        ValueType::Quantity(name) => {
            let (actual, unit) = crate::units::state_unit(state)?;
            if actual != name {
                return None;
            }
            let canonical = quantity::to_canonical(finite_number(text)?, unit);
            canonical.is_finite().then(|| canonical.into())
        }
    }
}

pub fn unit_mismatch(state: &State, value_type: ValueType) -> bool {
    matches!(value_type, ValueType::Quantity(name)
        if crate::units::state_unit(state).is_none_or(|(actual, _)| actual != name))
}

pub fn type_property() -> ticknode_plugin_sdk::PropertyDefinition {
    use ticknode_plugin_sdk::PropertyDefinition;
    let mut property = PropertyDefinition::select(
        "value_type",
        "Value type",
        &[
            ("number", "Number"),
            ("bool", "On/Off"),
            ("text", "Text"),
            ("datetime", "Date and time"),
        ],
    )
    .described("Auto-detected when choosing a known entity; can be overridden");
    if let ticknode_plugin_sdk::PropertyKind::Select(options) = &mut property.kind {
        options.extend(
            quantity::catalog()
                .iter()
                .map(|d| (d.name.clone(), d.title.clone())),
        );
    }
    property
}
