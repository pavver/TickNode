//! Button builder node (`telegram/button`).
use crate::{
    runtime, schema,
    validation::{self, Properties},
};
use ticknode_plugin_sdk::*;

pub struct ButtonNode {
    props: Properties,
    pub error: Option<UiError>,
}

impl ButtonNode {
    pub fn new() -> Self {
        Self {
            props: schema::defaults("button"),
            error: None,
        }
    }
}

impl PluginNode for ButtonNode {
    fn definition(&self) -> NodeDefinition {
        schema::definition("button")
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        self.definition()
            .properties
            .into_iter()
            .map(|p| {
                let value = self.props.get(&p.name).cloned().unwrap_or_default();
                (p.name, value.into())
            })
            .collect()
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::String(text) = value {
            self.props.insert(name.into(), text);
        }
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        let DataValue::String(text) = value else {
            return Err(validation::error("property", "Expected a text property"));
        };
        validation::property("button", name, text, &self.props)
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        self.error = None;
        let Some(DataValue::String(text)) = inputs.get(PinId(1)) else {
            return vec![];
        };
        if text.trim().is_empty() {
            return vec![];
        }

        let action = self.props.get("action").map_or("callback", |s| s.as_str());
        let data_in = match inputs.get(PinId(2)) {
            Some(DataValue::String(s)) => Some(s.as_str()),
            _ => None,
        };

        let data = match action {
            "callback" => {
                let d = match data_in {
                    Some(s) if !s.is_empty() => s,
                    _ => text.as_str(),
                };
                Some(d.to_string())
            }
            "url" => {
                let Some(url) = data_in.filter(|s| !s.trim().is_empty()) else {
                    let err = validation::error("button-url", "A URL is required for URL buttons");
                    self.error = Some(err.clone());
                    runtime::report_error(&err, true);
                    return vec![];
                };
                Some(url.to_string())
            }
            _ => None,
        };

        let btn = StructValue::new("telegram.button")
            .field("text", text.clone())
            .field("action", action)
            .field_opt("data", data)
            .build();

        vec![Output::Data(PinId(0), btn)]
    }
}
