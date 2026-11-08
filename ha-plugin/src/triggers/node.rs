//! Trigger node properties, lifecycle and error polling.

use std::collections::BTreeMap;
use std::time::Instant;
use ticknode_plugin_sdk::{Trigger as NodeTrigger, *};
use crate::{text_property, try_with_hub, with_hub};
use super::{Platform, build};

pub struct TriggerNode {
    node: NodeId,
    serial: Option<u64>,
    platform: Platform,
    properties: BTreeMap<String, String>,
    local_error: Option<String>,
}

impl TriggerNode {
    pub fn new(platform: Platform, node: NodeId) -> Self {
        let properties = platform.defaults();
        let serial = (!node.0.is_nil()).then(|| {
            with_hub(|hub, env| {
                if platform == Platform::State {
                    hub.set_state_node_entity(node, "", env);
                }
                let serial = hub.add_trigger(node, Instant::now());
                hub.set_trigger(node, serial, build(platform, &properties).ok().flatten(), Instant::now(), env);
                serial
            })
        });
        Self { node, serial, platform, properties, local_error: None }
    }
}

impl Drop for TriggerNode {
    fn drop(&mut self) {
        if self.platform == Platform::State {
            try_with_hub(|hub, env| hub.remove_state_node(self.node, env));
        }
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_trigger(self.node, serial, Instant::now(), env));
        }
    }
}

impl PluginNode for TriggerNode {
    fn definition(&self) -> NodeDefinition {
        let mut def = self.platform.definition();
        if self.platform == Platform::State {
            let entity = self.properties.get("entity_id").map(String::as_str).unwrap_or("");
            let state_src = crate::states::state_source(entity);
            let attr_src = crate::states::attribute_source(entity);
            for prop in &mut def.properties {
                match prop.name.as_str() {
                    "from" | "to" => prop.kind = PropertyKind::Dynamic { source: state_src.clone(), plugin: String::new() },
                    "attribute" => prop.kind = PropertyKind::Dynamic { source: attr_src.clone(), plugin: String::new() },
                    _ => {}
                }
            }
        }
        def
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.properties.iter().map(|(k, v)| (k.clone(), DataValue::from(v.as_str()))).collect()
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else { return };
        let Some(current) = self.properties.get(name) else { return };
        if current == &text {
            return;
        }
        if let Some(def) = self.platform.properties().iter().find(|p| p.name == name)
            && let PropertyKind::Select(options) = &def.kind
            && !options.iter().any(|(v, _)| v == &text)
        {
            return;
        }
        self.properties.insert(name.into(), text.clone());
        if self.platform == Platform::State && self.serial.is_some() && name == "entity_id" {
            with_hub(|hub, env| hub.set_state_node_entity(self.node, &text, env));
        }
        self.local_error = None;
        let config = match build(self.platform, &self.properties) {
            Ok(config) => config,
            Err(error) => {
                self.local_error = Some(error);
                None
            }
        };
        if let Some(serial) = self.serial {
            with_hub(|hub, env| hub.set_trigger(self.node, serial, config, Instant::now(), env));
        }
    }
    fn execute(&mut self, _: NodeTrigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
    fn polls(&self) -> bool {
        true
    }
    fn poll(&mut self, _: u64) -> Option<PinId> {
        if let Some(error) = self.local_error.take() {
            host::error(&error);
        }
        if let Some(serial) = self.serial {
            let errors = with_hub(|hub, _| hub.take_trigger_errors(self.node, serial));
            for error in errors {
                host::error(&error);
            }
        }
        None
    }
}
