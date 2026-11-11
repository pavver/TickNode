//! Service schema discovery and Home Assistant selector/target conversion.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use ticknode_plugin_sdk::DataType;

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub data_type: DataType,
    pub required: bool,
    /// HA unit captured with the field; absent in legacy Number snapshots.
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityFilter {
    pub domains: Vec<String>,
    pub device_classes: Vec<String>,
    pub supported_features: Vec<u64>,
}

impl EntityFilter {
    pub fn matches(&self, entity_domain: &str, state: Option<&ha_core_client::State>) -> bool {
        if !self.domains.is_empty() && !self.domains.iter().any(|d| d == entity_domain) {
            return false;
        }
        if !self.device_classes.is_empty()
            && let Some(state) = state
            && !state
                .attributes
                .get_as::<String>("device_class")
                .is_some_and(|class| self.device_classes.iter().any(|c| c == &class))
        {
            return false;
        }
        if !self.supported_features.is_empty()
            && let Some(state) = state
            && let Some(features) = state.attributes.get_as::<u64>("supported_features")
            && !self
                .supported_features
                .iter()
                .any(|&mask| (features & mask) != 0)
        {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug, Default)]
pub struct Service {
    pub label: String,
    pub fields: Vec<Field>,
    pub domains: Vec<String>,
    pub filters: Vec<EntityFilter>,
    pub targeted: bool,
    pub response: bool,
}

pub fn selector_type(selector: &Value) -> DataType {
    let has = |key| selector.get(key).is_some();
    if has("number") || has("color_temp") {
        selector_unit(selector).map_or(DataType::Number, |(name, _)| DataType::Named(name.into()))
    } else if has("boolean") {
        DataType::Bool
    } else if has("text") || has("template") || has("select") {
        DataType::String
    } else if has("color_rgb") {
        DataType::Color
    } else if has("time") {
        DataType::Time
    } else if has("duration") {
        DataType::Duration
    } else {
        DataType::Any
    }
}

fn selector_unit(
    selector: &Value,
) -> Option<(
    &'static str,
    &'static ticknode_plugin_sdk::quantity::UnitDefinition,
)> {
    if let Some(config) = selector.get("color_temp") {
        let kind = ticknode_plugin_sdk::quantity::kind("quantity.color_temperature")?;
        let id = config["unit"].as_str().unwrap_or("mired");
        return kind
            .units
            .iter()
            .find(|u| u.id == id)
            .map(|u| ("quantity.color_temperature", u));
    }
    let symbol = selector["number"]["unit_of_measurement"].as_str()?;
    // Number selectors keep HA's numeric duration contract.
    if crate::units::duration_factor(symbol).is_some() {
        return None;
    }
    crate::units::mapping(symbol, selector["number"]["device_class"].as_str())
}

fn humanize(name: &str) -> String {
    let text = name.replace('_', " ");
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

fn value_strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) if !s.is_empty() => vec![s.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn value_u64s(value: &Value) -> Vec<u64> {
    match value {
        Value::Number(n) => n.as_u64().into_iter().collect(),
        Value::Array(items) => items.iter().filter_map(Value::as_u64).collect(),
        _ => Vec::new(),
    }
}

pub fn entity_filters(domain: &str, service: &Value) -> Vec<EntityFilter> {
    if domain == "homeassistant" || !service.get("target").is_some_and(Value::is_object) {
        return Vec::new();
    }
    let items: Vec<&Value> = match &service["target"]["entity"] {
        Value::Array(filters) => filters.iter().collect(),
        filter @ Value::Object(_) => vec![filter],
        _ => Vec::new(),
    };
    let mut result = Vec::new();
    for filter in items {
        result.push(entity_filter(domain, filter));
    }
    if result.is_empty() && !domain.is_empty() {
        result.push(EntityFilter {
            domains: vec![domain.to_string()],
            device_classes: Vec::new(),
            supported_features: Vec::new(),
        });
    }
    result
}

/// HA's target filters are alternatives; each filter's domain can be a string or a list.
pub fn target_domains(domain: &str, service: &Value) -> Vec<String> {
    let mut domains = BTreeSet::new();
    for filter in entity_filters(domain, service) {
        domains.extend(filter.domains);
    }
    domains.into_iter().filter(|d| !d.is_empty()).collect()
}

pub fn services(json: &Value) -> BTreeMap<String, Service> {
    let mut result = BTreeMap::new();
    let Some(domains) = json.as_object() else {
        return result;
    };
    for (domain, services) in domains {
        let Some(services) = services.as_object() else {
            continue;
        };
        for (id, service) in services {
            let mut fields = Vec::new();
            if let Some(schema) = service["fields"].as_object() {
                // Deterministic order, with advanced fields last. Once selected, only the snapshot defines pins.
                let mut entries: Vec<_> = schema.iter().collect();
                entries.sort_by_key(|(name, field)| {
                    (field["advanced"].as_bool().unwrap_or(false), *name)
                });
                for (name, field) in entries {
                    fields.push(Field {
                        name: name.clone(),
                        label: field["name"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(name)
                            .into(),
                        data_type: selector_type(&field["selector"]),
                        required: field["required"].as_bool().unwrap_or(false),
                        unit: selector_unit(&field["selector"]).map(|(_, unit)| {
                            field["selector"]["number"]["unit_of_measurement"]
                                .as_str()
                                .or_else(|| field["selector"]["color_temp"]["unit"].as_str())
                                .unwrap_or(&unit.symbol)
                                .to_string()
                        }),
                    });
                }
            }
            result.insert(
                format!("{domain}.{id}"),
                Service {
                    label: service["name"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .unwrap_or_else(|| humanize(id)),
                    fields,
                    domains: target_domains(domain, service),
                    filters: entity_filters(domain, service),
                    targeted: service.get("target").is_some_and(Value::is_object),
                    response: service.get("response").is_some_and(Value::is_object),
                },
            );
        }
    }
    result
}

fn entity_filter(domain: &str, filter: &Value) -> EntityFilter {
    let mut domains = value_strings(&filter["domain"]);
    if domains.is_empty() && !domain.is_empty() {
        domains.push(domain.to_string());
    }
    EntityFilter {
        domains,
        device_classes: value_strings(&filter["device_class"]),
        supported_features: value_u64s(&filter["supported_features"]),
    }
}
