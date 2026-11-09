//! Property-backed action form, snapshots and service execution.

use super::{
    DONE, ERROR, FAILED, Field, RESPONSE, Service, action_source, field_json, outputs,
    service_matches,
};
use crate::{CATEGORY, text_property, with_hub};
use serde_json::Value;
use std::time::Instant;
use ticknode_plugin_sdk::*;
use ticknode_shared::{data_type_name, parse_data_type};

pub(super) fn strings(values: impl IntoIterator<Item = String>) -> DataValue {
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

pub struct ActionNode {
    node: NodeId,
    pub(super) action: String,
    target: String,
    pub(super) service: Service,
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
            adaptive: false,
            cost: 50,
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
            "action" => self.set_action(value),
            "target" => self.set_target(value),
            "fields" => self.set_fields(value),
            "field_labels" => self.set_field_labels(value),
            "required_fields" => self.set_required_fields(value),
            "target_domains" => self.service.domains = read_strings(value),
            "supports_response" => self.set_supports_response(value),
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

impl ActionNode {
    fn set_action(&mut self, value: DataValue) {
        if let Some(action) = text_property(value) {
            if action != self.action {
                self.service = Service::default();
            }
            self.action = action;
            if let Some(service) = with_hub(|hub, _| hub.services.get(&self.action).cloned()) {
                self.service = service;
            }
        }
    }

    fn set_target(&mut self, value: DataValue) {
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

    fn set_fields(&mut self, value: DataValue) {
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

    fn set_field_labels(&mut self, value: DataValue) {
        for (field, label) in self.service.fields.iter_mut().zip(read_strings(value)) {
            field.label = label;
        }
    }

    fn set_required_fields(&mut self, value: DataValue) {
        let required = read_strings(value);
        for field in &mut self.service.fields {
            field.required = required.contains(&field.name);
        }
    }

    fn set_supports_response(&mut self, value: DataValue) {
        if let DataValue::Bool(response) = value {
            self.service.response = response;
        }
    }
}
