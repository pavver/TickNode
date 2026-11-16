//! Compose message node (`telegram/compose`).
use crate::{
    runtime, schema,
    validation::{self, Properties},
};
use ticknode_plugin_sdk::*;

pub struct ComposeNode {
    props: Properties,
    pub error: Option<UiError>,
}

impl ComposeNode {
    pub fn new() -> Self {
        Self {
            props: schema::defaults("compose"),
            error: None,
        }
    }
}

impl PluginNode for ComposeNode {
    fn definition(&self) -> NodeDefinition {
        schema::definition("compose")
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        validation::node_properties(self.definition(), &self.props)
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        validation::set_text_property(&mut self.props, name, value);
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        validation::node_property("compose", name, value, &self.props)
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        self.error = None;
        let text_input = match inputs.get(PinId(1)) {
            Some(DataValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let media_input = inputs.get(PinId(2));

        let keyboard_input = inputs.get(PinId(3));
        let parse_mode = self.props.get("parse_mode").map_or("plain", |s| s.as_str());
        let silent = self.props.get("silent").map_or("no", |s| s.as_str()) == "yes";

        let mut out = StructValue::new("telegram.outgoing")
            .field_opt("text", text_input)
            .field_opt("media", media_input.cloned())
            .field_opt("keyboard", keyboard_input.cloned());

        if parse_mode != "plain" {
            out = out.field("parse_mode", parse_mode);
        }
        if silent {
            out = out.field("silent", true);
        }

        let value = out.build();
        if let Err(error) = crate::outgoing::parse_outgoing(&value) {
            self.error = Some(error.clone());
            runtime::report_error(&error);
            return vec![];
        }
        vec![Output::Data(PinId(0), value)]
    }
}
