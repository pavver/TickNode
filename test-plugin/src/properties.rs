//! Typed schema and validation probes for the host contract.
use ticknode_plugin_sdk::*;

pub struct Properties {
    values: Vec<(String, DataValue)>,
}
impl Default for Properties {
    fn default() -> Self {
        Self {
            values: ["duration", "time", "days", "template", "secret", "custom"]
                .into_iter()
                .map(|n| (n.into(), "".into()))
                .collect(),
        }
    }
}
impl PluginNode for Properties {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/properties".into(),
            title: "Properties".into(),
            category: "Test".into(),
            description: "Typed property validation probe".into(),
            help: "Exercises the host property contract.".into(),
            cost: 1,
            adaptive: false,
            pins: vec![],
            properties: vec![
                PropertyDefinition::duration("duration", "Duration", true),
                PropertyDefinition::time("time", "Time"),
                PropertyDefinition::multi_select(
                    "days",
                    "Days",
                    &[("mon", "Monday"), ("tue", "Tuesday")],
                ),
                PropertyDefinition::multiline("template", "Template"),
                PropertyDefinition::hidden("secret"),
                PropertyDefinition::text("custom", "Custom"),
            ],
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.values.clone()
    }
    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        if name == "custom" {
            if *value == DataValue::from("reject") {
                return Err(
                    UiError::new("test-plugin-property-rejected", "Property rejected")
                        .arg("name", name),
                );
            }
            if *value == DataValue::from("foreign") {
                return Err(UiError::new("error-node-not-found", "Foreign key"));
            }
        }
        Ok(())
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if let Some((_, current)) = self.values.iter_mut().find(|(n, _)| n == name) {
            *current = value;
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        vec![]
    }
}
