//! Keyboard builder node (`telegram/keyboard`).
use crate::{
    runtime, schema,
    validation::{self, Properties},
};
use ticknode_plugin_sdk::*;

pub struct KeyboardNode {
    props: Properties,
    pub error: Option<UiError>,
}

impl KeyboardNode {
    pub fn new() -> Self {
        Self {
            props: schema::defaults("keyboard"),
            error: None,
        }
    }
}

impl PluginNode for KeyboardNode {
    fn definition(&self) -> NodeDefinition {
        schema::action_definition("keyboard", &self.props)
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        validation::node_properties(self.definition(), &self.props)
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        validation::set_text_property(&mut self.props, name, value);
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        validation::node_property("keyboard", name, value, &self.props)
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        self.error = None;
        let kind = self.props.get("kind").map_or("inline", |s| s.as_str());
        if kind == "remove" {
            let kb = StructValue::new("telegram.keyboard")
                .field("kind", "remove")
                .field(
                    "rows",
                    DataValue::List {
                        element_type: DataType::List(Box::new(schema::named("button"))),
                        elements: vec![],
                    },
                )
                .build();
            return vec![Output::Data(PinId(0), kb)];
        }

        let count: u32 = self
            .props
            .get("count")
            .and_then(|s| s.parse().ok())
            .unwrap_or(3)
            .clamp(1, 24);
        let columns: usize = self
            .props
            .get("columns")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1)
            .clamp(1, 8);

        let mut buttons = Vec::new();
        for i in 1..=count {
            match inputs
                .get(PinId(i))
                .map(|value| keyboard_button(value, kind))
                .transpose()
            {
                Ok(Some(Some(button))) => buttons.push(button),
                Ok(_) => {}
                Err(error) => {
                    self.error = Some(error.clone());
                    runtime::report_error(&error);
                    return vec![];
                }
            }
        }

        let mut rows: Vec<DataValue> = Vec::new();
        for chunk in buttons.chunks(columns) {
            rows.push(DataValue::List {
                element_type: schema::named("button"),
                elements: chunk.to_vec(),
            });
        }
        let rows_val = DataValue::List {
            element_type: DataType::List(Box::new(schema::named("button"))),
            elements: rows,
        };

        let mut kb = StructValue::new("telegram.keyboard")
            .field("kind", kind)
            .field("rows", rows_val);

        if kind == "reply" {
            let resize = self.props.get("resize").map_or("yes", |s| s.as_str()) == "yes";
            let one_time = self.props.get("one_time").map_or("no", |s| s.as_str()) == "yes";
            let persistent = self.props.get("persistent").map_or("no", |s| s.as_str()) == "yes";
            let placeholder = self
                .props
                .get("placeholder")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty());

            kb = kb
                .field("resize", resize)
                .field("one_time", one_time)
                .field("persistent", persistent)
                .field_opt("placeholder", placeholder.map(String::from));
        }

        vec![Output::Data(PinId(0), kb.build())]
    }
}

fn keyboard_button(value: &DataValue, kind: &str) -> Result<Option<DataValue>, UiError> {
    match value {
        DataValue::String(text) => {
            if text.trim().is_empty() {
                return Ok(None);
            }
            let (action, data) = if kind == "inline" {
                ("callback", Some(text.clone()))
            } else {
                ("text", None)
            };
            Ok(Some(
                StructValue::new("telegram.button")
                    .field("text", text.clone())
                    .field("action", action)
                    .field_opt("data", data)
                    .build(),
            ))
        }
        DataValue::Object { .. } => object_button(value, kind),
        _ => Ok(None),
    }
}

fn object_button(value: &DataValue, kind: &str) -> Result<Option<DataValue>, UiError> {
    let text = field_string(value, "text").unwrap_or("");
    if text.trim().is_empty() {
        return Ok(None);
    }
    let action = field_string(value, "action").unwrap_or("callback");
    let error = match kind {
        "inline" if matches!(action, "text" | "contact" | "location") => Some((
            "telegram-error-inline-action",
            format!("Inline keyboard cannot use '{action}' action for button '{text}'"),
        )),
        "reply" if matches!(action, "callback" | "url") => Some((
            "telegram-error-reply-action",
            format!("Reply keyboard cannot use '{action}' action for button '{text}'"),
        )),
        _ => None,
    };
    if let Some((key, fallback)) = error {
        return Err(UiError::new(key, fallback)
            .arg("text", text)
            .arg("action", action));
    }
    Ok(Some(value.clone()))
}
