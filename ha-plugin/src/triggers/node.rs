//! Trigger node properties, lifecycle and error polling.

use super::{Platform, build};
use crate::{text_property, try_with_hub, with_hub};
use std::collections::BTreeMap;
use std::time::Instant;
use ticknode_plugin_sdk::{Trigger as NodeTrigger, *};

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
                let serial = hub.add_trigger(node, Instant::now());
                hub.set_trigger(
                    node,
                    serial,
                    build(platform, &properties).ok().flatten(),
                    Instant::now(),
                    env,
                );
                serial
            })
        });
        Self {
            node,
            serial,
            platform,
            properties,
            local_error: None,
        }
    }
}

impl Drop for TriggerNode {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_trigger(self.node, serial, Instant::now(), env));
        }
    }
}

impl PluginNode for TriggerNode {
    fn definition(&self) -> NodeDefinition {
        self.platform.definition()
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
        let Some(current) = self.properties.get(name) else {
            return;
        };
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
