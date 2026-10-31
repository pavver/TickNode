//! Service discovery and the property-backed action form. Snapshots preserve positional pins offline.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde_json::{Value, json};
use ticknode_plugin_sdk::*;
use ticknode_shared::{data_type_name, parse_data_type};

use crate::convert::{json_to_value, value_to_json};
use crate::icons::domain_icon;
use crate::{CATEGORY, text_property, with_hub};

// Reserve the high IDs so service fields can grow from input pin 2 without moving outputs.
pub const DONE: PinId = PinId(u32::MAX - 3);
pub const FAILED: PinId = PinId(u32::MAX - 2);
pub const ERROR: PinId = PinId(u32::MAX - 1);
pub const RESPONSE: PinId = PinId(u32::MAX);

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub label: String,
    pub data_type: DataType,
    pub required: bool,
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
            && !self.supported_features.iter().any(|&mask| (features & mask) != 0)
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
    if has("number") {
        DataType::Number
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
        let mut domains = value_strings(&filter["domain"]);
        if domains.is_empty() && !domain.is_empty() {
            domains.push(domain.to_string());
        }
        result.push(EntityFilter {
            domains,
            device_classes: value_strings(&filter["device_class"]),
            supported_features: value_u64s(&filter["supported_features"]),
        });
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

pub fn action_source(target: &str) -> String {
    let target = target.trim();
    if target.is_empty() {
        "actions".into()
    } else {
        format!("actions:{target}")
    }
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

/// Whether `service` can be executed for `target` (`Some((entity_id, state))` when an entity is selected,
/// or `None` for actions that run without an entity).
pub fn service_matches(
    service: &Service,
    target: Option<(&str, Option<&ha_core_client::State>)>,
) -> bool {
    match target {
        Some((entity_id, state)) => {
            let domain = crate::icons::domain_of(entity_id.trim());
            !domain.is_empty()
                && service.targeted
                && service.filters.iter().any(|f| f.matches(domain, state))
        }
        None => !service.targeted,
    }
}

/// Usage-frequency rank of a Home Assistant action (smaller = more frequently used).
pub fn action_rank(id: &str) -> u16 {
    let (domain, name) = id.split_once('.').unwrap_or(("", id));
    match name {
        "turn_on" | "open_cover" | "open_valve" | "lock" => 0,
        "turn_off" | "close_cover" | "close_valve" | "unlock" => 1,
        "stop_cover" | "stop_valve" => 2,
        "toggle" | "open" => 3,
        "close" | "stop" => 4,
        "press" => 5,
        "trigger" => 6,
        "activate" | "apply" => 7,
        "set_value" => 20,
        "select_option" => 21,
        "set_datetime" => 22,
        "set_temperature" => 23,
        "set_hvac_mode" => 24,
        "set_preset_mode" => 25,
        "set_percentage" => 26,
        "set_cover_position" | "set_valve_position" => 27,
        "set_humidity" => 28,
        "set_fan_mode" | "set_swing_mode" | "set_direction" => 29,
        "play_media" | "media_play_pause" => 40,
        "media_play" => 41,
        "media_pause" => 42,
        "media_stop" => 43,
        "volume_set" => 44,
        "volume_up" | "volume_down" | "volume_mute" => 45,
        "media_next_track" | "media_previous_track" => 46,
        "select_source" | "select_sound_mode" => 47,
        "start" | "start_mowing" => 60,
        "pause" => 61,
        "cancel" => 62,
        "finish" => 63,
        "return_to_base" | "dock" => 64,
        "increment" => 65,
        "decrement" => 66,
        "reset" => 67,
        "arm_away" | "alarm_arm_away" => 70,
        "arm_home" | "alarm_arm_home" => 71,
        "arm_night" | "alarm_arm_night" => 72,
        "disarm" | "alarm_disarm" => 73,
        "send_message" | "notify" | "speak" => 80,
        "create" | "dismiss" => 81,
        "add_item" | "update_item" | "remove_item" | "get_items" => 82,
        "get_forecasts" | "get_events" => 83,
        _ if name.starts_with("set_") || name.starts_with("select_") => 39,
        _ if name.starts_with("open_") || name.starts_with("close_") => 19,
        "update_entity" => 200,
        _ if name == "reload" || name.starts_with("reload_") => 201,
        _ if matches!(domain, "script" | "notify") => 90,
        _ => 100,
    }
}

pub fn options_for(
    services: &BTreeMap<String, Service>,
    target: Option<(&str, Option<&ha_core_client::State>)>,
) -> Vec<PropertyOption> {
    let entity_domain = target.map(|(entity_id, _)| crate::icons::domain_of(entity_id.trim()));
    let mut options: Vec<_> = services
        .iter()
        .filter(|(_, service)| service_matches(service, target))
        .map(|(id, service)| PropertyOption {
            value: id.clone(),
            label: service.label.clone(),
            detail: id.clone(),
            icon: Some(domain_icon(crate::icons::domain_of(id)).into()),
        })
        .collect();
    options.sort_by_key(|o| {
        let service_domain = crate::icons::domain_of(&o.value);
        let tier = match entity_domain {
            Some(d) if service_domain == d => 0u8,
            Some(_) => 1u8,
            None => 0u8,
        };
        (tier, action_rank(&o.value), o.label.to_lowercase(), o.value.clone())
    });
    options
}

fn strings(values: impl IntoIterator<Item = String>) -> DataValue {
    DataValue::List {
        element_type: DataType::String,
        elements: values.into_iter().map(DataValue::String).collect(),
    }
}

fn read_strings(value: DataValue) -> Vec<String> {
    match value {
        DataValue::List { elements, .. } => elements
            .into_iter()
            .filter_map(|v| match v {
                DataValue::String(s) => Some(s),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn field_json(value: &DataValue) -> Value {
    match value {
        DataValue::Color(c) => json!([c.r, c.g, c.b]),
        DataValue::Duration(d) => {
            json!({"hours": d.as_secs() / 3600, "minutes": d.as_secs() / 60 % 60, "seconds": d.as_secs_f64() % 60.0})
        }
        other => value_to_json(other),
    }
}

pub fn outputs(result: Result<Value, String>) -> Vec<Output> {
    match result {
        Ok(result) => {
            let mut outputs = vec![Output::Data(ERROR, "".into()), Output::Exec(DONE)];
            if let Some(response) = result
                .get("response")
                .and_then(json_to_value)
                .filter(|v| matches!(v, DataValue::Object { .. }))
            {
                outputs.push(Output::Data(RESPONSE, response));
            }
            outputs
        }
        Err(e) => vec![Output::Data(ERROR, e.into()), Output::Exec(FAILED)],
    }
}

pub struct ActionNode {
    node: NodeId,
    action: String,
    target: String,
    service: Service,
}

impl ActionNode {
    pub fn new(node: NodeId) -> Self {
        Self {
            node,
            action: String::new(),
            target: String::new(),
            service: Service::default(),
        }
    }
}

impl Drop for ActionNode {
    fn drop(&mut self) {
        if !self.node.0.is_nil() {
            crate::try_with_hub(|hub, env| hub.remove_action_target(self.node, env));
        }
    }
}

impl PluginNode for ActionNode {
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![exec_in(0, "In"), data_in(1, "Entity", DataType::String)];
        pins.extend(self.service.fields.iter().enumerate().map(|(i, f)| {
            let pin = data_in(i as u32 + 2, &f.label, f.data_type.clone());
            if f.required { pin.required() } else { pin }
        }));
        pins.extend([
            exec_out(DONE.0, "Done"),
            exec_out(FAILED.0, "Failed"),
            data_out(ERROR.0, "Error", DataType::String),
            data_out(RESPONSE.0, "Response", DataType::Object),
        ]);
        NodeDefinition {
            type_name: "ha/action".into(), title: "Action".into(), category: CATEGORY.into(),
            description: "Runs a Home Assistant action with fields from its form".into(),
            help: "Choose an entity and an action from the lists: the action list is filtered by the selected entity and sorted by usage frequency. Entity overrides the selected entity when set. Selecting an action while its catalog is available snapshots its fields as input pins (advanced fields last); they survive saving and reconnects. Required fields must be set; unset optional fields are omitted. Select fields accept text; their options are not shown as a dropdown. Colors are sent as RGB, durations as hours/minutes/seconds, and times as HH:MM:SS. Done confirms success; Failed and Error report a rejected request, timeout or disconnected connection. Response is emitted only when HA returns response data. Reselect the action to refresh its field schema after HA changes it.".into(),
            properties: vec![
                PropertyDefinition::dynamic("target", "Entity", crate::entities::ACTION_TARGETS_SOURCE)
                    .described("The target entity; filters the available actions. The Entity input overrides this choice"),
                PropertyDefinition::dynamic("action", "Action", &action_source(&self.target))
                    .described("The Home Assistant action to run on the selected entity"),
            ], pins,
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("target".into(), self.target.clone().into()),
            ("action".into(), self.action.clone().into()),
            (
                "fields".into(),
                strings(
                    self.service
                        .fields
                        .iter()
                        .map(|f| format!("{}:{}", f.name, data_type_name(&f.data_type))),
                ),
            ),
            (
                "field_labels".into(),
                strings(self.service.fields.iter().map(|f| f.label.clone())),
            ),
            (
                "required_fields".into(),
                strings(
                    self.service
                        .fields
                        .iter()
                        .filter(|f| f.required)
                        .map(|f| f.name.clone()),
                ),
            ),
            (
                "target_domains".into(),
                strings(self.service.domains.clone()),
            ),
            (
                "supports_response".into(),
                DataValue::Bool(self.service.response),
            ),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        match name {
            "action" => {
                if let Some(action) = text_property(value) {
                    if action != self.action {
                        self.service = Service::default();
                    }
                    self.action = action;
                    if let Some(service) =
                        with_hub(|hub, _| hub.services.get(&self.action).cloned())
                    {
                        self.service = service;
                    }
                }
            }
            "target" => {
                if let Some(target) = text_property(value) {
                    let changed = target != self.target;
                    self.target = target;
                    let clear = with_hub(|hub, env| {
                        if !self.node.0.is_nil() {
                            hub.set_action_target(self.node, &self.target, env);
                        }
                        let target_ctx = (!self.target.is_empty())
                            .then(|| (self.target.as_str(), hub.live.get(&self.target)));
                        changed
                            && !self.action.is_empty()
                            && hub
                                .services
                                .get(&self.action)
                                .is_some_and(|s| !service_matches(s, target_ctx))
                    });
                    if clear {
                        self.action.clear();
                        self.service = Service::default();
                    }
                }
            }
            "fields" => {
                self.service.fields = read_strings(value)
                    .into_iter()
                    .filter_map(|s| {
                        let (name, kind) = s.split_once(':')?;
                        if name.is_empty() {
                            return None;
                        }
                        Some(Field {
                            name: name.into(),
                            label: name.into(),
                            data_type: parse_data_type(kind)?,
                            required: false,
                        })
                    })
                    .collect();
            }
            "field_labels" => {
                for (field, label) in self.service.fields.iter_mut().zip(read_strings(value)) {
                    field.label = label;
                }
            }
            "required_fields" => {
                let required = read_strings(value);
                for field in &mut self.service.fields {
                    field.required = required.contains(&field.name);
                }
            }
            "target_domains" => self.service.domains = read_strings(value),
            "supports_response" => {
                if let DataValue::Bool(response) = value {
                    self.service.response = response;
                }
            }
            _ => {}
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let Some((domain, service)) = self
            .action
            .split_once('.')
            .filter(|(d, s)| !d.is_empty() && !s.is_empty())
        else {
            return outputs(Err("Choose a Home Assistant action (domain.service)".into()));
        };
        let target = match inputs.get(PinId(1)) {
            Some(DataValue::String(s)) => s.trim(),
            _ => &self.target,
        };
        let data: serde_json::Map<_, _> = self
            .service
            .fields
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                Some((f.name.clone(), field_json(inputs.get(PinId(i as u32 + 2))?)))
            })
            .collect();
        with_hub(|hub, env| {
            hub.action(
                self.node,
                domain,
                service,
                target,
                Value::Object(data),
                self.service.response,
                Instant::now(),
                env,
            )
        })
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Value {
        json!({
            "light": {"turn_on": {"name":"Turn on", "target":{"entity":[{"domain":"light"}]}, "fields":{
                "brightness":{"name":"Brightness", "required":true, "selector":{"number":{"min":0,"max":255}}},
                "rgb_color":{"selector":{"color_rgb":null}},
                "effect":{"selector":{"select":{"options":["none","rainbow"]}}},
                "transition":{"advanced":true,"selector":{"number":{}}}
            }}},
            "weather":{"get_forecasts":{"target":{"entity":[{"domain":["weather","sensor"]}]},"response":{"optional":false}}},
            "homeassistant":{"turn_on":{"target":{}}},
            "script":{"run_script":{}}
        })
    }

    #[test]
    fn service_catalog_builds_options_fields_targets_and_response_support() {
        let services = services(&catalog());
        let light = &services["light.turn_on"];
        assert_eq!(
            light
                .fields
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["brightness", "effect", "rgb_color", "transition"]
        );
        assert_eq!(
            light.fields[0],
            Field {
                name: "brightness".into(),
                label: "Brightness".into(),
                data_type: DataType::Number,
                required: true
            }
        );
        assert_eq!(action_source(""), "actions");
        assert_eq!(action_source("light.kitchen"), "actions:light.kitchen");
        assert!(!light.response);
        assert!(services["weather.get_forecasts"].response);
        assert_eq!(services["weather.get_forecasts"].domains, ["sensor", "weather"]);
        assert!(services["homeassistant.turn_on"].targeted);
        assert!(services["homeassistant.turn_on"].domains.is_empty());
        assert!(!services["script.run_script"].targeted);
        let light_options = options_for(&services, Some(("light.kitchen", None)));
        assert_eq!(
            light_options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            ["light.turn_on"]
        );
        let light = &light_options[0];
        assert_eq!(
            (
                light.label.as_str(),
                light.detail.as_str(),
                light.icon.as_deref()
            ),
            ("Turn on", "light.turn_on", Some("mdi:lightbulb"))
        );
        let sensor_options = options_for(&services, Some(("sensor.temp", None)));
        assert_eq!(
            sensor_options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            ["weather.get_forecasts"]
        );
        assert_eq!(sensor_options[0].label, "Get forecasts");
        let targetless = options_for(&services, None);
        assert_eq!(
            targetless.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            ["script.run_script"]
        );
        assert_eq!(target_domains("switch", &json!({"target":{}})), ["switch"]);
        assert!(!services.is_empty());
        assert!(super::services(&Value::Null).is_empty());
    }

    #[test]
    fn entity_actions_filter_by_supported_features_and_device_class() {
        let services = services(&json!({
            "cover": {
                "open_cover": {"name": "Open cover", "target": {"entity": [{"domain": "cover", "supported_features": [1]}]}},
                "close_cover": {"name": "Close cover", "target": {"entity": [{"domain": "cover", "supported_features": [2]}]}},
                "set_cover_tilt_position": {"name": "Set tilt", "target": {"entity": [{"domain": "cover", "supported_features": [128]}]}},
                "open_shutter": {"name": "Open shutter", "target": {"entity": [{"domain": "cover", "device_class": ["shutter"]}]}}
            }
        }));
        let blind = crate::entities::test_state(
            "cover.blind",
            "open",
            json!({"supported_features": 3, "device_class": "blind"}),
        );
        let shutter = crate::entities::test_state(
            "cover.shutter",
            "closed",
            json!({"supported_features": 131, "device_class": "shutter"}),
        );
        let blind_actions: Vec<_> = options_for(&services, Some(("cover.blind", Some(&blind))))
            .into_iter()
            .map(|o| o.value)
            .collect();
        assert_eq!(blind_actions, ["cover.open_cover", "cover.close_cover"]);
        let shutter_actions: Vec<_> =
            options_for(&services, Some(("cover.shutter", Some(&shutter))))
                .into_iter()
                .map(|o| o.value)
                .collect();
        assert_eq!(
            shutter_actions,
            [
                "cover.open_cover",
                "cover.close_cover",
                "cover.open_shutter",
                "cover.set_cover_tilt_position",
            ]
        );
    }

    #[test]
    fn actions_are_sorted_by_usage_frequency_before_alphabetical_order() {
        let services = services(&json!({
            "light": {
                "toggle": {"name": "Toggle", "target": {"entity": [{"domain": "light"}]}},
                "turn_off": {"name": "Turn off", "target": {"entity": [{"domain": "light"}]}},
                "turn_on": {"name": "Turn on", "target": {"entity": [{"domain": "light"}]}},
                "reload": {"name": "Reload", "target": {"entity": [{"domain": "light"}]}}
            },
            "cover": {
                "stop_cover": {"name": "Stop cover", "target": {"entity": [{"domain": "cover"}]}},
                "close_cover": {"name": "Close cover", "target": {"entity": [{"domain": "cover"}]}},
                "set_cover_position": {"name": "Set cover position", "target": {"entity": [{"domain": "cover"}]}},
                "open_cover": {"name": "Open cover", "target": {"entity": [{"domain": "cover"}]}},
                "toggle": {"name": "Toggle", "target": {"entity": [{"domain": "cover"}]}}
            }
        }));
        let light: Vec<_> = options_for(&services, Some(("light.kitchen", None)))
            .into_iter()
            .map(|o| o.value)
            .collect();
        assert_eq!(
            light,
            [
                "light.turn_on",
                "light.turn_off",
                "light.toggle",
                "light.reload",
            ]
        );
        let cover: Vec<_> = options_for(&services, Some(("cover.hall", None)))
            .into_iter()
            .map(|o| o.value)
            .collect();
        assert_eq!(
            cover,
            [
                "cover.open_cover",
                "cover.close_cover",
                "cover.stop_cover",
                "cover.toggle",
                "cover.set_cover_position",
            ]
        );
    }

    #[test]
    fn selectors_map_to_pin_types() {
        for (selector, ty) in [
            ("number", DataType::Number),
            ("boolean", DataType::Bool),
            ("text", DataType::String),
            ("template", DataType::String),
            ("select", DataType::String),
            ("color_rgb", DataType::Color),
            ("time", DataType::Time),
            ("duration", DataType::Duration),
            ("object", DataType::Any),
            ("entity", DataType::Any),
        ] {
            assert_eq!(selector_type(&json!({selector:null})), ty, "{selector}");
        }
        assert_eq!(selector_type(&Value::Null), DataType::Any);
    }

    #[test]
    fn action_snapshot_restores_the_exact_form_without_a_catalog() {
        let mut action = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
        crate::with_hub(|hub, _| hub.services = services(&catalog()));
        action.set_property("target", "light.kitchen".into());
        action.set_property("action", "light.turn_on".into());
        let props = action.properties();
        assert_eq!(props[0], ("target".into(), "light.kitchen".into()));
        assert_eq!(props[1], ("action".into(), "light.turn_on".into()));
        assert_eq!(
            props.iter().find(|(k, _)| k == "fields").unwrap().1,
            strings(
                [
                    "brightness:number",
                    "effect:string",
                    "rgb_color:color",
                    "transition:number"
                ]
                .map(str::to_string)
            )
        );
        let def = action.definition();
        assert_eq!(def.properties[0].name, "target");
        assert_eq!(
            def.properties[0].kind,
            PropertyKind::Dynamic {
                source: crate::entities::ACTION_TARGETS_SOURCE.into(),
                plugin: String::new()
            }
        );
        assert_eq!(
            def.properties[1].kind,
            PropertyKind::Dynamic {
                source: "actions:light.kitchen".into(),
                plugin: String::new()
            }
        );
        assert_eq!(
            (
                def.pins[2].id,
                def.pins[2].name.as_str(),
                def.pins[2].required
            ),
            (PinId(2), "Brightness", true)
        );
        assert_eq!(
            def.pins[3].pin_type,
            ticknode_shared::PinType::Data(DataType::String)
        );
        // Switching to another entity of the same domain keeps the selected action and its fields.
        action.set_property("target", "light.bedroom".into());
        assert_eq!(action.action, "light.turn_on");
        assert_eq!(action.service.fields.len(), 4);
        // Switching to an entity of an incompatible domain resets the action while the catalog is loaded.
        action.set_property("target", "weather.home".into());
        assert!(action.action.is_empty());
        assert!(action.service.fields.is_empty());
        action.set_property("target", "light.kitchen".into());
        action.set_property("action", "light.turn_on".into());
        // Clearing the target entity resets an entity-targeted action while the catalog is loaded.
        action.set_property("target", "".into());
        assert!(action.action.is_empty());
        assert!(action.service.fields.is_empty());
        action.set_property("target", "light.kitchen".into());
        action.set_property("action", "light.turn_on".into());
        crate::with_hub(|hub, _| hub.services.clear());
        let mut restored = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
        for (name, value) in props.clone() {
            restored.set_property(&name, value);
        }
        assert_eq!(restored.definition(), def);
        assert_eq!(restored.properties(), props);
        restored.set_property("action", "script.run_script".into());
        assert!(restored.service.fields.is_empty());
        assert_eq!(restored.definition().pins.len(), 6);
    }

    #[test]
    fn field_values_are_ha_selector_values() {
        assert_eq!(
            field_json(&DataValue::Color(ticknode_shared::Color {
                r: 12,
                g: 34,
                b: 56,
                a: 255
            })),
            json!([12, 34, 56])
        );
        assert_eq!(
            field_json(&DataValue::Duration(std::time::Duration::from_millis(
                3_661_500
            ))),
            json!({"hours":1,"minutes":1,"seconds":1.5})
        );
        assert_eq!(
            field_json(&DataValue::Time(
                chrono::NaiveTime::from_hms_opt(7, 30, 5).unwrap()
            )),
            json!("07:30:05")
        );
        assert_eq!(field_json(&DataValue::from(120i64)), json!(120));
        assert_eq!(field_json(&DataValue::Bool(false)), json!(false));
        assert_eq!(field_json(&DataValue::from("rainbow")), json!("rainbow"));
    }

    #[test]
    fn response_is_emitted_only_for_response_data() {
        let out = outputs(Ok(
            json!({"context":{},"response":{"weather.home":{"forecast":[]}}}),
        ));
        assert!(
            out.iter()
                .any(|o| matches!(o,Output::Data(p,DataValue::Object{..}) if *p == RESPONSE))
        );
        for value in [Value::Null, json!({"response":null}), json!({"context":{}})] {
            assert!(
                !outputs(Ok(value))
                    .iter()
                    .any(|o| matches!(o,Output::Data(p,_) if *p == RESPONSE))
            );
        }
        assert!(
            outputs(Err("rejected".into()))
                .iter()
                .any(|o| matches!(o,Output::Exec(p) if *p == FAILED))
        );
    }
}
