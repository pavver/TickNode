//! Condition node properties, lifecycle and execution.

use super::{Kind, evaluate, outputs};
use crate::{text_property, try_with_hub, with_hub};
use chrono::Utc;
use std::collections::BTreeMap;
use std::time::Instant;
use ticknode_plugin_sdk::*;

pub struct ConditionNode {
    node: NodeId,
    serial: Option<u64>,
    kind: Kind,
    properties: BTreeMap<String, String>,
    warning: Option<(u64, String)>,
    invalid_reported: Option<UiError>,
}
impl ConditionNode {
    pub fn new(kind: Kind, node: NodeId) -> Self {
        let properties = kind
            .definition()
            .properties
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    if kind == Kind::Sun && matches!(p.name.as_str(), "after" | "before") {
                        "none".into()
                    } else {
                        String::new()
                    },
                )
            })
            .collect();
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_condition(node)));
        Self {
            node,
            serial,
            kind,
            properties,
            warning: None,
            invalid_reported: None,
        }
    }
    fn warn(&mut self, generation: u64, error: String) {
        let warning = (generation, error);
        if self.warning.as_ref() != Some(&warning) {
            host::warn(&format!(
                "{}: {}; returning No",
                self.kind.type_name(),
                warning.1
            ));
            self.warning = Some(warning);
        }
    }
}
impl Drop for ConditionNode {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_condition(self.node, serial, Instant::now(), env));
        }
    }
}
impl PluginNode for ConditionNode {
    fn definition(&self) -> NodeDefinition {
        let mut def = self.kind.definition();
        if self.kind == Kind::Sun {
            for property in &mut def.properties {
                if let Some(boundary) = property.name.strip_suffix("_offset")
                    && self
                        .properties
                        .get(boundary)
                        .is_none_or(|v| v == "none" || v.is_empty())
                {
                    property.kind = PropertyKind::Hidden;
                }
            }
        }
        def
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.kind
            .definition()
            .properties
            .into_iter()
            .filter_map(|p| {
                self.properties
                    .get(&p.name)
                    .map(|v| (p.name, DataValue::from(v.as_str())))
            })
            .collect()
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else {
            return;
        };
        let Some(old) = self.properties.get(name) else {
            return;
        };
        if old == &text {
            return;
        }
        self.properties.insert(name.into(), text.clone());
        self.warning = None;
        if self.configuration_error().is_none() {
            self.invalid_reported = None;
        }
        if let Some(serial) = self.serial {
            with_hub(|hub, env| hub.cancel_templates(self.node, serial, Instant::now(), env));
        }
    }
    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        crate::validation::condition(self.kind, name, value)
    }
    fn polls(&self) -> bool {
        self.configuration_error().is_some()
    }
    fn poll(&mut self, _: u64) -> Option<PinId> {
        self.report_configuration_error();
        None
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        if self.report_configuration_error() {
            return Vec::new();
        }
        if self.kind == Kind::Template {
            return with_hub(|hub, env| {
                hub.render_condition(
                    self.node,
                    self.serial.unwrap_or_default(),
                    &self.properties["value_template"],
                    Instant::now(),
                    env,
                )
            });
        }
        let (generation, result) = with_hub(|hub, _| {
            (
                hub.live.generation,
                evaluate(self.kind, &self.properties, &hub.live, Utc::now()),
            )
        });
        match result {
            Ok(value) => {
                self.warning = None;
                outputs(value)
            }
            Err(error) => {
                self.warn(generation, error);
                outputs(false)
            }
        }
    }
}

impl ConditionNode {
    fn configuration_error(&self) -> Option<UiError> {
        let def = self.definition();
        self.properties.iter().find_map(|(name, value)| {
            if def
                .properties
                .iter()
                .any(|p| p.name == *name && p.kind == PropertyKind::Hidden)
            {
                return None;
            }
            self.validate_property(name, &DataValue::from(value.as_str()))
                .err()
        })
    }
    fn report_configuration_error(&mut self) -> bool {
        let Some(error) = self.configuration_error() else {
            return false;
        };
        host::node_error(&error, self.invalid_reported.as_ref() != Some(&error));
        self.invalid_reported = Some(error);
        true
    }
}
