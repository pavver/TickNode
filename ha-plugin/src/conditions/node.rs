//! Condition node properties, lifecycle and execution.

use std::collections::BTreeMap;
use std::time::Instant;
use chrono::Utc;
use ticknode_plugin_sdk::*;
use crate::{text_property, try_with_hub, with_hub};
use super::{Kind, evaluate, outputs};

pub struct ConditionNode {
    node: NodeId,
    serial: Option<u64>,
    kind: Kind,
    properties: BTreeMap<String, String>,
    warning: Option<(u64, String)>,
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
        let serial = (!node.0.is_nil()).then(|| {
            with_hub(|hub, env| {
                if kind == Kind::State {
                    hub.set_state_node_entity(node, "", env);
                }
                hub.add_condition(node)
            })
        });
        Self {
            node,
            serial,
            kind,
            properties,
            warning: None,
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
        if self.kind == Kind::State {
            try_with_hub(|hub, env| hub.remove_state_node(self.node, env));
        }
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_condition(self.node, serial, Instant::now(), env));
        }
    }
}
impl PluginNode for ConditionNode {
    fn definition(&self) -> NodeDefinition {
        let mut def = self.kind.definition();
        if self.kind == Kind::State {
            let entity = self.properties.get("entity_id").map(String::as_str).unwrap_or("");
            let state_src = crate::states::state_source(entity);
            let attr_src = crate::states::attribute_source(entity);
            for prop in &mut def.properties {
                match prop.name.as_str() {
                    "state" => prop.kind = PropertyKind::Dynamic { source: state_src.clone(), plugin: String::new() },
                    "attribute" => prop.kind = PropertyKind::Dynamic { source: attr_src.clone(), plugin: String::new() },
                    _ => {}
                }
            }
        }
        def
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.properties
            .iter()
            .map(|(k, v)| (k.clone(), DataValue::from(v.as_str())))
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
        if let Some(def) = self
            .kind
            .definition()
            .properties
            .iter()
            .find(|p| p.name == name)
            && let PropertyKind::Select(options) = &def.kind
            && !options.iter().any(|(v, _)| v == &text)
        {
            return;
        }
        self.properties.insert(name.into(), text.clone());
        if self.kind == Kind::State && self.serial.is_some() && name == "entity_id" {
            with_hub(|hub, env| hub.set_state_node_entity(self.node, &text, env));
        }
        self.warning = None;
        if let Some(serial) = self.serial {
            with_hub(|hub, env| hub.cancel_templates(self.node, serial, Instant::now(), env));
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
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
