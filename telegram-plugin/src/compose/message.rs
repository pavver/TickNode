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
        validation::property("compose", name, text, &self.props)
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        self.error = None;
        let text_input = match inputs.get(PinId(1)) {
            Some(DataValue::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => None,
        };
        let media_input = inputs.get(PinId(2));

        if text_input.is_none() && media_input.is_none() {
            let err = validation::error("compose-empty", "Message requires either text or media");
            self.error = Some(err.clone());
            runtime::report_error(&err, true);
            return vec![];
        }

        if let Some(media_val) = media_input {
            let is_media = match media_val {
                DataValue::Object {
                    type_name: Some(name),
                    ..
                } => matches!(
                    name.as_str(),
                    "media.photo" | "media.audio" | "media.video" | "media.document"
                ),
                _ => false,
            };
            let file_id = field_string(media_val, "file_id").filter(|s| !s.trim().is_empty());
            let blob = field_string(media_val, "blob").filter(|s| !s.trim().is_empty());
            let url = field_string(media_val, "url").filter(|s| !s.trim().is_empty());
            if !is_media || (file_id.is_none() && blob.is_none() && url.is_none()) {
                let err =
                    validation::error("media", "A media value must have url, file_id, or blob set");
                self.error = Some(err.clone());
                runtime::report_error(&err, true);
                return vec![];
            }
            if let Some(t) = &text_input
                && t.chars().count() > 1024
            {
                let err =
                    validation::error("caption", "Telegram caption cannot exceed 1024 characters");
                self.error = Some(err.clone());
                runtime::report_error(&err, true);
                return vec![];
            }
        } else {
            let Some(t) = &text_input else {
                let err = validation::error("text", "Message text is required");
                self.error = Some(err.clone());
                runtime::report_error(&err, true);
                return vec![];
            };
            if t.is_empty() || t.chars().count() > 4096 {
                let err = validation::error(
                    "length",
                    "Telegram message text must contain 1–4096 characters",
                );
                self.error = Some(err.clone());
                runtime::report_error(&err, true);
                return vec![];
            }
        }

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

        vec![Output::Data(PinId(0), out.build())]
    }
}
