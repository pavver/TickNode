//! Trigger node properties, lifecycle and error polling.

use super::{Platform, build};
use crate::{try_with_hub, with_hub};
use std::collections::BTreeMap;
use std::time::Instant;
use ticknode_plugin_sdk::{Trigger as NodeTrigger, *};

pub struct TriggerNode {
    node: NodeId,
    serial: Option<u64>,
    platform: Platform,
    properties: BTreeMap<String, String>,
    local_error: Option<UiError>,
    reported: bool,
    /// The configuration is sent to the hub on the next poll, after every property of a restored
    /// node has been applied: sending each intermediate state could briefly subscribe a broader trigger.
    dirty: bool,
}

impl TriggerNode {
    pub fn new(platform: Platform, node: NodeId) -> Self {
        let properties = platform.defaults();
        let serial =
            (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_trigger(node, Instant::now())));
        Self {
            node,
            serial,
            platform,
            properties,
            local_error: None,
            reported: false,
            dirty: true,
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
        self.platform
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
        let DataValue::String(text) = value else {
            return;
        };
        let Some(current) = self.properties.get(name) else {
            return;
        };
        if current == &text {
            return;
        }
        self.properties.insert(name.into(), text.clone());
        let previous_error = self.local_error.clone();
        self.local_error = crate::validation::stored_trigger(self.platform, &self.properties).err();
        if let Err(error) = build(self.platform, &self.properties)
            && self.local_error.is_none()
        {
            self.local_error = Some(crate::validation::error("configuration", &error));
        }
        if self.local_error != previous_error {
            self.reported = false;
        }
        self.dirty = true;
    }
    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        crate::validation::trigger(self.platform, name, value)
    }
    fn execute(&mut self, _: NodeTrigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
    fn polls(&self) -> bool {
        true
    }
    fn poll(&mut self, _: u64) -> Option<PinId> {
        if std::mem::take(&mut self.dirty)
            && let Some(serial) = self.serial
        {
            let config = if self.local_error.is_none() {
                build(self.platform, &self.properties).ok().flatten()
            } else {
                None
            };
            with_hub(|hub, env| hub.set_trigger(self.node, serial, config, Instant::now(), env));
        }
        if let Some(error) = &self.local_error {
            host::node_error(error, !self.reported);
            self.reported = true;
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
