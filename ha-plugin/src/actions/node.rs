//! Property-backed action form, snapshots and service execution.

use super::{
    DONE, ERROR, FAILED, Field, RESPONSE, Service, action_source, outputs, service_matches,
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
    skip_unchanged: bool,
}

impl ActionNode {
    pub fn new(node: NodeId) -> Self {
        Self {
            node,
            action: String::new(),
            target: String::new(),
            service: Service::default(),
            skip_unchanged: true,
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
            help: "Choose an entity and an action from the lists: the action list is filtered by the selected entity and sorted by usage frequency. Choose From input to expose the Entity input; connecting it hides the entity picker. Choosing an entity removes the input. Old graphs with both a selected entity and a wire preserve that wire as a visible detached connection; comma-separated entity IDs address multiple targets. Selecting an action while its catalog is available snapshots its fields as input pins (advanced fields last); they survive saving and reconnects. Required fields must be set; unset optional fields are omitted. Select fields accept text; their options are not shown as a dropdown. Number fields with recognized units use quantity inputs and convert canonical values back to the snapshotted HA unit. Unitless number/input_number Value and climate Temperature selectors infer their unit from a single selected entity (or HA temperature settings). Legacy field snapshots stay unchanged. Set state actions take a required Boolean State: true turns on, locks or opens; false turns off, unlocks or closes. Skip if already set defaults to Yes, including existing graphs: recognized actions complete on Done without calling HA only when all targets are known and already satisfy the effect. Additional fields, unknown states and unresolved targets keep the normal call. Colors are sent as RGB, durations as hours/minutes/seconds, and times as HH:MM:SS. Done confirms success; Failed and Error report a rejected request, timeout or disconnected connection. Response is emitted only when HA returns response data. Reselect the action to refresh its field schema after HA changes it.".into(),
            properties: vec![
                PropertyDefinition::dynamic("target", "Entity", crate::entities::ACTION_TARGETS_SOURCE)
                    .or_input(1)
                    .described("Choose a target entity or From input; a selected entity filters the available actions"),
                PropertyDefinition::dynamic("action", "Action", &action_source(&self.target))
                    .described("The Home Assistant action to run on the selected entity"),
                PropertyDefinition::select("skip_unchanged", "Skip if already set", &[("yes", "Yes"), ("no", "No")])
                    .described("Complete successfully without calling HA when every target already has the desired state"),
                PropertyDefinition::hidden("supports_response"),
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
            (
                "field_units".into(),
                strings(
                    self.service
                        .fields
                        .iter()
                        .map(|f| f.unit.clone().unwrap_or_default()),
                ),
            ),
            (
                "skip_unchanged".into(),
                if self.skip_unchanged { "yes" } else { "no" }.into(),
            ),
        ]
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        if !matches!(name, "target" | "action" | "skip_unchanged") {
            return Ok(());
        }
        crate::validation::schema(&self.definition(), name, value)?;
        let DataValue::String(text) = value else {
            return Ok(());
        };
        let valid = match name {
            "target" => {
                text.is_empty()
                    || text
                        .split(',')
                        .all(|id| crate::validation::entity_id(id.trim()))
            }
            "action" => text.is_empty() || crate::validation::entity_id(text),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(crate::validation::error(
                "entity-id",
                "Enter an entity ID as domain.object_id",
            ))
        }
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
            "field_units" => self.set_field_units(value),
            "skip_unchanged" => match text_property(value).as_deref() {
                Some("yes") => self.skip_unchanged = true,
                Some("no") => self.skip_unchanged = false,
                _ => {}
            },
            _ => {}
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        with_hub(|hub, env| self.run(inputs, hub, env, Instant::now()))
    }
}

impl ActionNode {
    pub(crate) fn run(
        &self,
        inputs: &Inputs,
        hub: &mut crate::hub::Hub,
        env: &mut impl crate::hub::Env,
        now: Instant,
    ) -> Vec<Output> {
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
        let mut data = serde_json::Map::new();
        for (i, field) in self.service.fields.iter().enumerate() {
            if let Some(value) = inputs.get(PinId(i as u32 + 2)) {
                let Some(value) = field.json(value) else {
                    return outputs(Err(format!(
                        "Invalid unit conversion for field {}",
                        field.name
                    )));
                };
                data.insert(field.name.clone(), value);
            }
        }
        hub.action(
            self.node,
            domain,
            service,
            target,
            Value::Object(data),
            self.service.response,
            self.skip_unchanged,
            now,
            env,
        )
        .unwrap_or_default()
    }
    fn set_action(&mut self, value: DataValue) {
        if let Some(action) = text_property(value) {
            if action != self.action {
                self.service = Service::default();
            }
            self.action = action;
            if let Some(service) = with_hub(|hub, _| {
                let mut service = hub.services.get(&self.action).cloned().or_else(|| {
                    let (domain, name) = self.action.split_once('.')?;
                    (name == "set_state")
                        .then(|| super::effects::state_service(domain))
                        .flatten()
                })?;
                super::inference::infer_units(
                    &self.action,
                    &mut service,
                    hub.live.get(self.target.trim()),
                    hub.live.temperature_unit(),
                );
                Some(service)
            }) {
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
                    unit: None,
                })
            })
            .collect();
    }

    fn set_field_labels(&mut self, value: DataValue) {
        for (field, label) in self.service.fields.iter_mut().zip(read_strings(value)) {
            field.label = label;
        }
    }

    fn set_field_units(&mut self, value: DataValue) {
        for (field, unit) in self.service.fields.iter_mut().zip(read_strings(value)) {
            field.unit = (!unit.is_empty()).then_some(unit);
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
