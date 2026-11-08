//! State and attribute autocomplete options for state triggers and condition gates.

use ha_core_client::State;
use serde_json::Value;
use ticknode_plugin_sdk::PropertyOption;

mod canonical;

use canonical::domain_canonical_states;
use crate::icons::domain_of;

pub const STATES_SOURCE: &str = "states";
pub const ATTRIBUTES_SOURCE: &str = "attributes";

pub fn state_source(entity_id: &str) -> String {
    let entity_id = entity_id.trim();
    if entity_id.is_empty() {
        STATES_SOURCE.into()
    } else {
        format!("{STATES_SOURCE}:{entity_id}")
    }
}

pub fn attribute_source(entity_id: &str) -> String {
    let entity_id = entity_id.trim();
    if entity_id.is_empty() {
        ATTRIBUTES_SOURCE.into()
    } else {
        format!("{ATTRIBUTES_SOURCE}:{entity_id}")
    }
}

pub fn state_options(
    entity_id: &str,
    state: Option<&State>,
    allow_empty: bool,
) -> Vec<PropertyOption> {
    let mut options = Vec::new();
    if allow_empty {
        options.push(PropertyOption {
            value: String::new(),
            label: "Any state (not set)".into(),
            detail: "Triggers on any change".into(),
            icon: Some("mdi:circle-outline".into()),
        });
    }

    let domain = domain_of(entity_id.trim());
    let device_class = state.and_then(|s| s.attributes.get_as::<String>("device_class"));

    // If current state is known, add it prominently
    if let Some(s) = state
        && !s.state.is_empty()
        && s.state != "unavailable"
        && s.state != "unknown"
    {
        options.push(PropertyOption {
            value: s.state.clone(),
            label: format!("Current state ({})", s.state),
            detail: entity_id.to_string(),
            icon: Some("mdi:history".into()),
        });
    }

    // Add canonical domain states (skipping value if it matches the current state already added)
    for (value, label, icon) in domain_canonical_states(domain, device_class.as_deref()) {
        if !options.iter().any(|o| o.value == value) {
            options.push(PropertyOption {
                value: value.into(),
                label: label.into(),
                detail: format!("{domain}.{value}"),
                icon: icon.map(str::to_string),
            });
        }
    }

    // Universal special states
    for (value, label) in [
        ("unavailable", "Unavailable (unavailable)"),
        ("unknown", "Unknown (unknown)"),
    ] {
        if !options.iter().any(|o| o.value == value) {
            options.push(PropertyOption {
                value: value.into(),
                label: label.into(),
                detail: value.into(),
                icon: Some("mdi:help-circle-outline".into()),
            });
        }
    }

    options
}

pub fn attribute_options(_entity_id: &str, state: Option<&State>) -> Vec<PropertyOption> {
    let mut options = vec![PropertyOption {
        value: String::new(),
        label: "Main state (no attribute)".into(),
        detail: "Check entity state".into(),
        icon: Some("mdi:circle-outline".into()),
    }];

    if let Some(state) = state {
        let json = serde_json::to_value(&state.attributes).unwrap_or(Value::Null);
        let inner = match json {
            Value::Object(map) if map.len() == 1 => map.into_iter().next().map_or(Value::Null, |(_, v)| v),
            other => other,
        };
        if let Some(attrs) = inner.as_object() {
            let mut keys: Vec<_> = attrs.keys().collect();
            keys.sort();
            for key in keys {
                let val_str = match &attrs[key] {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                options.push(PropertyOption {
                    value: key.clone(),
                    label: key.clone(),
                    detail: format!("Value: {val_str}"),
                    icon: Some("mdi:code-tags".into()),
                });
            }
        }
    }

    options
}

#[cfg(test)]
mod tests;
