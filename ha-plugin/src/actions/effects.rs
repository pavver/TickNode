//! Conservative action effects and synthetic Boolean state actions.

use super::{EntityFilter, Field, Service};
use crate::live::Live;
use serde_json::Value;
use std::collections::BTreeMap;
use ticknode_plugin_sdk::DataType;

pub(super) struct StateAction {
    pub domain: &'static str,
    pub services: [&'static str; 2],
    pub states: [&'static str; 2],
    pub label: &'static str,
}

pub(super) const STATE_ACTIONS: &[StateAction] = &[
    on_off("switch"),
    on_off("light"),
    on_off("fan"),
    on_off("input_boolean"),
    on_off("siren"),
    on_off("humidifier"),
    on_off("automation"),
    on_off("remote"),
    StateAction {
        domain: "lock",
        services: ["unlock", "lock"],
        states: ["unlocked", "locked"],
        label: "Set locked",
    },
    StateAction {
        domain: "cover",
        services: ["close_cover", "open_cover"],
        states: ["closed", "open"],
        label: "Set open",
    },
];

const fn on_off(domain: &'static str) -> StateAction {
    StateAction {
        domain,
        services: ["turn_off", "turn_on"],
        states: ["off", "on"],
        label: "Set state (on/off)",
    }
}

pub(super) fn state_action(domain: &str) -> Option<&'static StateAction> {
    STATE_ACTIONS.iter().find(|effect| effect.domain == domain)
}

pub(super) fn state_service(domain: &str) -> Option<Service> {
    let effect = state_action(domain)?;
    Some(Service {
        label: effect.label.into(),
        fields: vec![Field {
            name: "state".into(),
            label: "State".into(),
            data_type: DataType::Bool,
            required: true,
            unit: None,
        }],
        domains: vec![domain.into()],
        filters: vec![EntityFilter {
            domains: vec![domain.into()],
            ..Default::default()
        }],
        targeted: true,
        ..Default::default()
    })
}

pub(super) fn add_state_services(services: &mut BTreeMap<String, Service>) {
    for effect in STATE_ACTIONS {
        if effect
            .services
            .iter()
            .any(|name| services.contains_key(&format!("{}.{name}", effect.domain)))
        {
            services.insert(
                format!("{}.set_state", effect.domain),
                state_service(effect.domain).unwrap(),
            );
        }
    }
}

pub(crate) fn resolve_state<'a>(
    domain: &str,
    service: &'a str,
    data: &mut Value,
) -> Result<&'a str, String> {
    if service != "set_state" {
        return Ok(service);
    }
    let Some(effect) = state_action(domain) else {
        return Ok(service);
    };
    let state = data
        .get("state")
        .and_then(Value::as_bool)
        .ok_or_else(|| "State must be a Boolean for a set_state action".to_string())?;
    data.as_object_mut()
        .expect("state field belongs to an object")
        .remove("state");
    Ok(effect.services[usize::from(state)])
}

/// Only explicit entity IDs can be proved complete without HA's area/device/label registries.
pub(crate) fn entity_ids(target: &str) -> Option<Vec<&str>> {
    let ids: Vec<_> = target.split(',').map(str::trim).collect();
    ids.iter()
        .all(|id| {
            id.split_once('.').is_some_and(|(domain, name)| {
                !domain.is_empty()
                    && !name.is_empty()
                    && id
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
            })
        })
        .then_some(ids)
}

#[derive(Clone, Copy)]
enum Effect {
    Number(&'static str),
    Text(&'static str),
    Attribute(&'static str),
}

const FIELD_EFFECTS: &[(&str, &str, Effect)] = &[
    ("number", "set_value", Effect::Number("value")),
    ("input_number", "set_value", Effect::Number("value")),
    ("select", "select_option", Effect::Text("option")),
    ("input_select", "select_option", Effect::Text("option")),
    ("input_text", "set_value", Effect::Text("value")),
    ("text", "set_value", Effect::Text("value")),
    (
        "climate",
        "set_temperature",
        Effect::Attribute("temperature"),
    ),
];

fn equal_number(actual: f64, desired: f64) -> bool {
    actual.is_finite()
        && desired.is_finite()
        && (actual - desired).abs() <= 1e-6 + 1e-9 * actual.abs().max(desired.abs())
}

pub(crate) fn unchanged(
    live: &Live,
    domain: &str,
    service: &str,
    target: &str,
    data: &Value,
) -> bool {
    let Some(ids) = entity_ids(target) else {
        return false;
    };
    let Some(fields) = data.as_object() else {
        return false;
    };
    let state_effect = state_action(domain).or_else(|| (domain == "climate").then_some(&CLIMATE));
    let desired = state_effect.and_then(|effect| {
        effect
            .services
            .iter()
            .position(|s| *s == service)
            .map(|index| effect.states[index])
    });
    let field_effect = FIELD_EFFECTS
        .iter()
        .find(|(d, s, _)| *d == domain && *s == service)
        .map(|(_, _, e)| *e);
    ids.iter().all(|id| {
        // A mismatched domain cannot prove this service's effect on the target.
        if crate::icons::domain_of(id) != domain {
            return false;
        }
        let Ok(state) = live.state(id) else {
            return false;
        };
        if let Some(desired) = desired {
            return fields.is_empty() && state.state == desired;
        }
        let Some(effect) = field_effect else {
            return false;
        };
        let (Effect::Number(key) | Effect::Text(key) | Effect::Attribute(key)) = effect;
        if fields.len() != 1 {
            return false;
        }
        match effect {
            Effect::Text(_) => data[key]
                .as_str()
                .is_some_and(|desired| state.state == desired),
            Effect::Number(_) => state
                .state
                .parse::<f64>()
                .ok()
                .zip(data[key].as_f64())
                .is_some_and(|(actual, desired)| equal_number(actual, desired)),
            Effect::Attribute(_) => state
                .attributes
                .get_as::<f64>(key)
                .zip(data[key].as_f64())
                .is_some_and(|(actual, desired)| equal_number(actual, desired)),
        }
    })
}

// Climate states are usually HVAC modes: only literal on/off states are recognized.
const CLIMATE: StateAction = on_off("climate");
