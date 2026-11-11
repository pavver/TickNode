//! Trigger lifecycle. Restored properties are installed together on the next poll.
mod filter;
#[cfg(test)]
mod tests;
use crate::{
    runtime, schema,
    validation::{self, Properties},
};
pub use filter::Filter;
use ticknode_plugin_sdk::*;

pub struct TriggerNode {
    kind: String,
    node: NodeId,
    props: Properties,
    dirty: bool,
    serial: Option<u64>,
    error: Option<UiError>,
    reported: bool,
}
impl TriggerNode {
    pub fn new(kind: &str, node: NodeId) -> Self {
        Self {
            kind: kind.into(),
            node,
            props: schema::defaults(kind),
            dirty: true,
            serial: None,
            error: None,
            reported: false,
        }
    }
}
impl Drop for TriggerNode {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            runtime::remove(self.node, serial);
        }
    }
}
impl PluginNode for TriggerNode {
    fn definition(&self) -> NodeDefinition {
        schema::definition(&self.kind)
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        schema::definition(&self.kind)
            .properties
            .into_iter()
            .map(|p| {
                let value = self.props.get(&p.name).cloned().unwrap_or_default();
                (p.name, value.into())
            })
            .collect()
    }
    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        let DataValue::String(text) = value else {
            return Err(validation::error("property", "Expected a text property"));
        };
        validation::property(&self.kind, name, text, &self.props)
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::String(mut text) = value
            && self.props.contains_key(name)
        {
            if name == "command" {
                text = text.strip_prefix('/').unwrap_or(&text).to_string();
            }
            if let Some(serial) = self.serial.take() {
                runtime::remove(self.node, serial);
            }
            self.props.insert(name.into(), text);
            self.dirty = true;
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
    fn polls(&self) -> bool {
        true
    }
    fn poll(&mut self, _: u64) -> Option<PinId> {
        if std::mem::take(&mut self.dirty) {
            if let Some(serial) = self.serial.take() {
                runtime::remove(self.node, serial);
            }
            self.error = None;
            self.reported = false;
            match Filter::new(&self.kind, self.props.clone()) {
                Ok(filter) if !self.node.0.is_nil() => {
                    self.serial = Some(runtime::with(|hub, _| hub.register(self.node, filter)))
                }
                Err(error) => self.error = Some(error),
                _ => {}
            }
        }
        if let Some(error) = &self.error {
            host::node_error(error, !self.reported);
            self.reported = true;
        }
        None
    }
}
