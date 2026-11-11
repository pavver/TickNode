#[cfg(test)]
mod formatting_tests;
#[cfg(test)]
mod tests;

use crate::{Node, NodeContext};
use ticknode_shared::{
    template,
    to_text::{ToTextOptions, to_text_with_identifiers},
    *,
};

const HELP: &str = r#"Formats a MiniJinja template. Undeclared variables become optional Any inputs in order of first appearance; their pin IDs survive edits. A text.name reference also creates the name input. The name text is reserved for the global presentation map and cannot be an input; do not assign to it in templates. Missing inputs and their text entries are undefined: {{ name | default('—') }} supplies a fallback.

Examples: Power: {{ text.power }}; {% if power > 1000 %}high{% endif %}; {{ (power / 1000) | round(1) }} kW; {% for item in items %}{{ item }} {% endfor %}; {{ name | default('—') }}.

Inputs are plain Jinja values, so arithmetic, comparisons, tests and standard filters work normally. The global text map formats each input exactly like To text, using the connected source's declared type, project display rules and engine formatting language: {{ text.power }} can render 1.2 kW. Lists and structures retain their declared element and field types in this presentation. Units come from text.name.

The text filter formats runtime kinds only, including values computed in the template: numbers use plain-number display rules, booleans use localized words, lists and maps are formatted recursively, and strings stay unchanged. It has no source type or quantity units: {{ (power / 1000) | text }} formats a plain number. Globals and filters have separate namespaces, so both text.name and | text are available.

Numbers retain integer/float precision; date-times use RFC 3339, and time, duration and color use canonical text. Structures expose field names. Pure node: evaluated when Text is needed. Rendering has a fixed 10,000 instruction fuel budget and the default recursion limit. A rendering error produces no output and reports a node error."#;

pub struct TemplateNode {
    id: NodeId,
    template: String,
    /// Parsed once per edit; `None` while the template has a syntax error.
    compiled: Option<template::CompiledTemplate>,
    pins: Vec<(String, PinId)>,
    next_pin: u32,
    restoring: bool,
    restoration_started: bool,
    reported: bool,
}

impl TemplateNode {
    pub fn new(id: NodeId) -> Self {
        let mut node = Self {
            id,
            template: "{{ value }}".into(),
            compiled: None,
            pins: Vec::new(),
            next_pin: 1,
            restoring: true,
            restoration_started: false,
            reported: false,
        };
        node.compile();
        node
    }

    fn compile(&mut self) {
        self.compiled = template::CompiledTemplate::new(&self.template).ok();
        let Ok(ordered) = template::variables(&self.template) else {
            return;
        };
        let old = std::mem::take(&mut self.pins);
        self.next_pin = old
            .iter()
            .map(|(_, pin)| pin.0 + 1)
            .max()
            .unwrap_or(1)
            .max(self.next_pin);
        self.pins = ordered
            .into_iter()
            .filter_map(|name| {
                let pin = if let Some((_, pin)) = old.iter().find(|(n, _)| *n == name) {
                    *pin
                } else {
                    // Do not allocate from the split-pin range or wrap around.
                    if self.next_pin >= 0x8000_0000 {
                        return None;
                    }
                    let pin = PinId(self.next_pin);
                    self.next_pin += 1;
                    pin
                };
                Some((name, pin))
            })
            .collect();
    }

    fn render(&self, ctx: &NodeContext) -> Result<String, template::TemplateError> {
        let inputs: Vec<(String, Option<DataValue>, DataType)> = self
            .pins
            .iter()
            .map(|(name, pin)| {
                let value = ctx.inputs.get(pin).and_then(Option::as_ref).cloned();
                let ty = ctx.input_types.get(pin).cloned().unwrap_or(DataType::Any);
                (name.clone(), value, ty)
            })
            .collect();
        let Some(compiled) = &self.compiled else {
            return template::validate(&self.template).map(|()| String::new());
        };
        let display = ctx.host_state.display.clone();
        let labels = crate::quantity::labels(&ctx.host_state.locale);
        compiled.render(
            &inputs,
            &|value, declared| {
                to_text_with_identifiers(
                    value,
                    declared,
                    &ToTextOptions::default(),
                    &ctx.host_state.display,
                    labels,
                    ctx.types,
                    &ctx.host_state.identifier_labels,
                )
            },
            std::sync::Arc::new(move |value: &DataValue| {
                to_text_with_identifiers(
                    value,
                    &DataType::Any,
                    &ToTextOptions::default(),
                    &display,
                    labels,
                    &TypeRegistry::new(),
                    &|_: &str, _: i64| None,
                )
            }),
        )
    }
}

impl Node for TemplateNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        let mut pins: Vec<_> = self
            .pins
            .iter()
            .map(|(name, id)| PinDefinition {
                id: *id,
                name: name.clone(),
                pin_type: PinType::Data(DataType::Any),
                direction: PinDirection::Input,
                required: false,
            })
            .collect();
        pins.push(PinDefinition {
            id: PinId(0),
            name: "Text".into(),
            pin_type: PinType::Data(DataType::String),
            direction: PinDirection::Output,
            required: false,
        });
        NodeDefinition {
            type_name: "text/template".into(),
            title: "Format text".into(),
            category: "String".into(),
            description: "Formats text with a MiniJinja template".into(),
            help: HELP.into(),
            properties: vec![
                PropertyDefinition::multiline("template", "Template")
                    .described("MiniJinja template; variables become input pins."),
                PropertyDefinition::hidden("pin_ids"),
                PropertyDefinition::hidden("next_pin"),
            ],
            pins,
            adaptive: true,
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        match self.render(ctx) {
            Ok(text) => {
                self.reported = false;
                vec![NodeOutput::Data(PinId(0), text.into())]
            }
            Err(error) => {
                let message = UiMessage::new(
                    "error-template-render",
                    format!("Template rendering failed: {error}"),
                )
                .arg("message", error.to_string());
                if self.reported {
                    ctx.host_state.report_error(self.id, message);
                } else {
                    ctx.host_state.error(
                        self.id,
                        message.clone(),
                        format!("[ERROR] {}", message.fallback),
                    );
                    self.reported = true;
                }
                vec![]
            }
        }
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
        if name == "template"
            && let DataValue::String(source) = value
        {
            template::validate(source).map_err(|error| {
                UiMessage::new(
                    "error-template-syntax",
                    format!("Invalid template syntax: {error}"),
                )
                .arg("message", error.to_string())
            })?;
        }
        Ok(())
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("template".into(), self.template.clone().into()),
            (
                "pin_ids".into(),
                DataValue::List {
                    element_type: DataType::String,
                    elements: self
                        .pins
                        .iter()
                        .map(|(n, p)| format!("{}:{n}", p.0).into())
                        .collect(),
                },
            ),
            (
                "next_pin".into(),
                DataValue::Number(NumberValue::Int(self.next_pin.into())),
            ),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if self.restoring && !self.restoration_started {
            // Constructor pins belong to the default template, not to the stored contract.
            // Discard them before restoring properties, regardless of their order.
            self.pins.clear();
            self.next_pin = 1;
            self.restoration_started = true;
        }
        match (name, value) {
            ("template", DataValue::String(source)) => self.template = source,
            ("pin_ids", DataValue::List { elements, .. }) => {
                let mut pins = Vec::new();
                for value in elements {
                    let DataValue::String(s) = value else {
                        continue;
                    };
                    let Some((pin, name)) = s.split_once(':') else {
                        continue;
                    };
                    let Ok(pin @ 1..=0x7fff_ffff) = pin.parse::<u32>() else {
                        continue;
                    };
                    if !pins.iter().any(|(n, p)| n == name || *p == PinId(pin)) {
                        pins.push((name.to_string(), PinId(pin)));
                    }
                }
                self.pins = pins;
            }
            ("next_pin", DataValue::Number(NumberValue::Int(n)))
                if (1..=0x8000_0000).contains(&n) =>
            {
                self.next_pin = self.next_pin.max(n as u32);
            }
            _ => return,
        }
        if !self.restoring {
            self.compile();
        }
    }

    fn restored(&mut self) {
        self.compile();
        self.restoring = false;
    }
}
