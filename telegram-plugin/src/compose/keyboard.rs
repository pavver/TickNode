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
        validation::property("keyboard", name, text, &self.props)
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

        let mut buttons: Vec<DataValue> = Vec::new();
        for i in 1..=count {
            let Some(val) = inputs.get(PinId(i)) else {
                continue;
            };
            match val {
                DataValue::String(s) => {
                    if s.trim().is_empty() {
                        continue;
                    }
                    let (action, data) = if kind == "inline" {
                        ("callback", Some(s.clone()))
                    } else {
                        ("text", None)
                    };
                    let btn = StructValue::new("telegram.button")
                        .field("text", s.clone())
                        .field("action", action)
                        .field_opt("data", data)
                        .build();
                    buttons.push(btn);
                }
                DataValue::Object { .. } => {
                    let text = field_string(val, "text").unwrap_or("");
                    if text.trim().is_empty() {
                        continue;
                    }
                    let action = field_string(val, "action").unwrap_or("callback");
                    if kind == "inline" && matches!(action, "text" | "contact" | "location") {
                        let err = UiError::new(
                            "telegram-error-inline-action",
                            format!(
                                "Inline keyboard cannot use '{action}' action for button '{text}'"
                            ),
                        )
                        .arg("text", text)
                        .arg("action", action);
                        self.error = Some(err.clone());
                        runtime::report_error(&err, true);
                        return vec![];
                    }
                    if kind == "reply" && matches!(action, "callback" | "url") {
                        let err = UiError::new(
                            "telegram-error-reply-action",
                            format!(
                                "Reply keyboard cannot use '{action}' action for button '{text}'"
                            ),
                        )
                        .arg("text", text)
                        .arg("action", action);
                        self.error = Some(err.clone());
                        runtime::report_error(&err, true);
                        return vec![];
                    }
                    buttons.push(val.clone());
                }
                _ => {}
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
