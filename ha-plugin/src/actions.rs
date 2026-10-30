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

#[derive(Clone, Debug, Default)]
pub struct Service {
    pub label: String,
    pub fields: Vec<Field>,
    pub domains: Vec<String>,
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

/// HA's target filters are alternatives; each filter's domain can be a string or a list.
pub fn target_domains(domain: &str, service: &Value) -> Vec<String> {
    if domain == "homeassistant" || !service.get("target").is_some_and(Value::is_object) {
        return Vec::new();
    }
    let mut domains = BTreeSet::new();
    if let Some(filters) = service["target"]["entity"].as_array() {
        for filter in filters {
            match &filter["domain"] {
                Value::String(d) => {
                    domains.insert(d.clone());
                }
                Value::Array(ds) => {
                    domains.extend(ds.iter().filter_map(Value::as_str).map(str::to_string))
                }
                _ => {}
            }
        }
    }
    if domains.is_empty() {
        domains.insert(domain.to_string());
    }
    domains.into_iter().filter(|d| !d.is_empty()).collect()
}

pub fn target_source(domains: &[String]) -> String {
    if domains.is_empty() {
        "entities".into()
    } else {
        format!("entities:{}", domains.join(","))
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
                    response: service.get("response").is_some_and(Value::is_object),
                },
            );
        }
    }
    result
}

pub fn options(services: &BTreeMap<String, Service>) -> Vec<PropertyOption> {
    let mut options: Vec<_> = services
        .iter()
        .map(|(id, service)| PropertyOption {
            value: id.clone(),
            label: service.label.clone(),
            detail: id.clone(),
            icon: Some(domain_icon(crate::icons::domain_of(id)).into()),
        })
        .collect();
    options.sort_by_key(|o| (o.label.to_lowercase(), o.value.clone()));
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
            help: "Choose an action and target from the lists. Entity overrides the target when set. Selecting an action while its catalog is available snapshots its fields as input pins (advanced fields last); they survive saving and reconnects. Required fields must be set; unset optional fields are omitted. Select fields accept text; their options are not shown as a dropdown. Colors are sent as RGB, durations as hours/minutes/seconds, and times as HH:MM:SS. Done confirms success; Failed and Error report a rejected request, timeout or disconnected connection. Response is emitted only when HA returns response data. Reselect the action to refresh its field schema after HA changes it.".into(),
            properties: vec![
                PropertyDefinition::dynamic("action", "Action", "actions").described("The Home Assistant action to run"),
                PropertyDefinition::dynamic("target", "Target", &target_source(&self.service.domains)).described("The target entity; the Entity input overrides this choice"),
            ], pins,
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("action".into(), self.action.clone().into()),
            ("target".into(), self.target.clone().into()),
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
                    self.target = target;
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
        assert_eq!(target_source(&light.domains), "entities:light");
        assert!(!light.response);
        assert!(services["weather.get_forecasts"].response);
        assert_eq!(
            target_source(&services["weather.get_forecasts"].domains),
            "entities:sensor,weather"
        );
        assert_eq!(
            target_source(&services["homeassistant.turn_on"].domains),
            "entities"
        );
        assert_eq!(
            target_source(&services["script.run_script"].domains),
            "entities"
        );
        let options = options(&services);
        let light = options.iter().find(|o| o.value == "light.turn_on").unwrap();
        assert_eq!(
            (
                light.label.as_str(),
                light.detail.as_str(),
                light.icon.as_deref()
            ),
            ("Turn on", "light.turn_on", Some("mdi:lightbulb"))
        );
        assert_eq!(
            options
                .iter()
                .find(|o| o.value == "weather.get_forecasts")
                .unwrap()
                .label,
            "Get forecasts"
        );
        assert_eq!(target_domains("switch", &json!({"target":{}})), ["switch"]);
        assert!(!services.is_empty());
        assert!(super::services(&Value::Null).is_empty());
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
        action.set_property("action", "light.turn_on".into());
        action.set_property("target", "light.kitchen".into());
        let props = action.properties();
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
