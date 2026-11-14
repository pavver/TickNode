//! Server-side quantity labels from simple embedded Fluent messages, cached per locale.
use fluent_syntax::{
    ast::{Entry, PatternElement},
    parser,
};
use std::{collections::BTreeMap, sync::OnceLock};
use ticknode_shared::{fluent_slug, quantity::QuantityLabels};

enum MessagePart {
    Text(String),
    Var(String),
}

struct MessagePattern {
    parts: Vec<MessagePart>,
    plain: Option<String>,
}

pub struct Labels {
    locale: String,
    messages: BTreeMap<String, MessagePattern>,
}

impl QuantityLabels for Labels {
    fn locale(&self) -> String {
        self.locale.clone()
    }
    fn unit_symbol(&self, id: &str) -> Option<String> {
        self.message(&format!("unit-{}", fluent_slug(id)))
    }
    fn bool_text(&self, value: bool) -> String {
        self.message(if value { "text-true" } else { "text-false" })
            .unwrap_or_else(|| value.to_string())
    }
    fn duration_unit(&self, unit: &str) -> String {
        self.message(&format!("text-duration-{unit}"))
            .unwrap_or_else(|| unit.into())
    }
    fn decimal_separator(&self) -> String {
        self.message("number-decimal-separator")
            .unwrap_or_else(|| ".".into())
    }
    fn message(&self, id: &str) -> Option<String> {
        self.messages.get(id).and_then(|p| p.plain.clone())
    }
    fn message_args(&self, id: &str, args: &[(&str, String)]) -> Option<String> {
        let pattern = self.messages.get(id)?;
        let mut out = String::new();
        for part in &pattern.parts {
            match part {
                MessagePart::Text(t) => out.push_str(t),
                MessagePart::Var(v) => {
                    if let Some((_, val)) = args.iter().find(|(k, _)| *k == v.as_str()) {
                        out.push_str(val);
                    }
                }
            }
        }
        Some(out)
    }
}

pub fn labels(locale: &str) -> &'static Labels {
    static EN: OnceLock<Labels> = OnceLock::new();
    static UK: OnceLock<Labels> = OnceLock::new();
    let (cache, source, loc) = if locale == "uk" {
        (&UK, crate::translations::CORE_TRANSLATIONS[1].1, "uk")
    } else {
        (&EN, crate::translations::CORE_TRANSLATIONS[0].1, "en")
    };
    cache.get_or_init(|| {
        let resource = parser::parse(source).expect("valid core translations");
        let messages = resource
            .body
            .into_iter()
            .filter_map(|e| {
                let Entry::Message(m) = e else { return None };
                let parts = m
                    .value?
                    .elements
                    .into_iter()
                    .map(|e| match e {
                        PatternElement::TextElement { value } => {
                            Some(MessagePart::Text(value.to_string()))
                        }
                        PatternElement::Placeable {
                            expression:
                                fluent_syntax::ast::Expression::Inline(
                                    fluent_syntax::ast::InlineExpression::StringLiteral { value },
                                ),
                        } => Some(MessagePart::Text(value.to_string())),
                        PatternElement::Placeable {
                            expression:
                                fluent_syntax::ast::Expression::Inline(
                                    fluent_syntax::ast::InlineExpression::VariableReference { id },
                                ),
                        } => Some(MessagePart::Var(id.name.to_string())),
                        _ => None,
                    })
                    .collect::<Option<Vec<MessagePart>>>()?;
                let is_plain = parts.iter().all(|p| matches!(p, MessagePart::Text(_)));
                let plain = if is_plain {
                    Some(
                        parts
                            .iter()
                            .map(|p| match p {
                                MessagePart::Text(t) => t.as_str(),
                                MessagePart::Var(_) => "",
                            })
                            .collect(),
                    )
                } else {
                    None
                };
                Some((m.id.name.to_string(), MessagePattern { parts, plain }))
            })
            .collect();
        Labels {
            locale: loc.to_string(),
            messages,
        }
    })
}
