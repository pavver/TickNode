#[cfg(test)]
mod formatting_tests;
#[cfg(test)]
mod tests;
mod values;

use crate::{Node, NodeContext};
use minijinja::{
    Environment, Value,
    machinery::{Token, tokenize},
};
use ticknode_shared::*;

const FUEL: u64 = 10_000;
const HELP: &str = r#"Formats a MiniJinja template. Undeclared variables become optional Any inputs in order of first appearance; their pin IDs survive edits. A text.name reference also creates the name input. The name text is reserved for the global presentation map and cannot be an input; do not assign to it in templates. Missing inputs and their text entries are undefined: {{ name | default('—') }} supplies a fallback.

Examples: Power: {{ text.power }}; {% if power > 1000 %}high{% endif %}; {{ (power / 1000) | round(1) }} kW; {% for item in items %}{{ item }} {% endfor %}; {{ name | default('—') }}.

Inputs are plain Jinja values, so arithmetic, comparisons, tests and standard filters work normally. The global text map formats each input exactly like To text, using the connected source's declared type, project display rules and engine formatting language: {{ text.power }} can render 1.2 kW. Lists and structures retain their declared element and field types in this presentation. Units come from text.name.

The text filter formats runtime kinds only, including values computed in the template: numbers use plain-number display rules, booleans use localized words, lists and maps are formatted recursively, and strings stay unchanged. It has no source type or quantity units: {{ (power / 1000) | text }} formats a plain number. Globals and filters have separate namespaces, so both text.name and | text are available.

Numbers retain integer/float precision; date-times use RFC 3339, and time, duration and color use canonical text. Structures expose field names. Pure node: evaluated when Text is needed. Rendering has a fixed 10,000 instruction fuel budget and the default recursion limit. A rendering error produces no output and reports a node error."#;

pub struct TemplateNode {
    id: NodeId,
    template: String,
    pins: Vec<(String, PinId)>,
    next_pin: u32,
    restoring: bool,
    env: Environment<'static>,
    reported: bool,
}

fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_fuel(Some(FUEL));
    // The render installs its own presentation-aware filter on a cheap environment clone.
    env.add_filter("text", |value: Value| value.to_string());
    // Globals and filters have separate namespaces; reserve the presentation map.
    env.add_global("text", Value::from_iter(Vec::<(String, Value)>::new()));
    env
}

impl TemplateNode {
    pub fn new(id: NodeId) -> Self {
        let mut node = Self {
            id,
            template: String::new(),
            pins: Vec::new(),
            next_pin: 1,
            restoring: true,
            env: environment(),
            reported: false,
        };
        node.compile();
        node
    }

    fn compile(&mut self) {
        self.env.remove_template("template");
        // Owned template storage avoids reparsing on every pure evaluation.
        let Ok(()) = self
            .env
            .add_template_owned("template", self.template.clone())
        else {
            return;
        };
        let template = self.env.get_template("template").unwrap();
        let mut names = template.undeclared_variables(false);
        // A presentation-only reference also declares its input, e.g. {{ text.power }}.
        names.extend(
            template
                .undeclared_variables(true)
                .iter()
                .filter_map(|name| {
                    name.strip_prefix("text.")
                        .and_then(|path| path.split('.').next())
                        .map(str::to_string)
                }),
        );
        names.retain(|name| !self.env.globals().any(|(global, _)| global == name));
        let mut ordered = Vec::new();
        let mut attribute = false;
        let mut tokens = tokenize(
            &self.template,
            false,
            Default::default(),
            Default::default(),
        )
        .peekable();
        while let Some(token) = tokens.next() {
            let Ok((token, _)) = token else { break };
            if let Token::Ident(mut name) = token
                && !attribute
            {
                if name == "text" && matches!(tokens.peek(), Some(Ok((Token::Dot, _)))) {
                    tokens.next();
                    if let Some(Ok((Token::Ident(field), _))) = tokens.next() {
                        name = field;
                    }
                }
                if names.remove(name) {
                    ordered.push(name.to_string());
                }
            }
            attribute = matches!(token, Token::Dot | Token::Pipe);
        }
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

    fn render(&self, ctx: &NodeContext) -> Result<String, minijinja::Error> {
        let context = self
            .pins
            .iter()
            .map(|(name, pin)| {
                let value = ctx.inputs.get(pin).and_then(Option::as_ref);
                (
                    name.clone(),
                    value.map(values::plain).unwrap_or(Value::UNDEFINED),
                )
            })
            .collect::<Value>();
        let mut env = self.env.clone();
        let text = self
            .pins
            .iter()
            .filter_map(|(name, pin)| {
                let value = ctx.inputs.get(pin)?.as_ref()?;
                let ty = ctx.input_types.get(pin).unwrap_or(&DataType::Any);
                Some((name.clone(), values::input_text(value, ty, ctx)))
            })
            .collect::<Value>();
        env.add_global("text", text);
        let presentation = values::presentation(ctx);
        env.add_filter("text", move |value: Value| {
            values::text_filter(&presentation, value)
        });
        env.get_template("template")?.render(context)
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
            environment().template_from_str(source).map_err(|error| {
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
