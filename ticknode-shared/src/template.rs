//! Pure MiniJinja template analysis, syntax validation and rendering.

use std::sync::Arc;
use std::ops::Range;
use minijinja::{
    Environment, Value,
    machinery::{Token, tokenize},
};
use serde::{Deserialize, Serialize};
use crate::{DataType, DataValue};

mod values;
#[cfg(test)]
mod tests;

pub mod complete;
pub use complete::{
    complete, expression_at, Completion, CompletionInput, CompletionItem, CompletionKind,
};

pub mod highlight;
pub use highlight::{highlight, Highlight, TokenClass};

pub use values::plain;

pub const FUEL: u64 = 10_000;

/// Information about a syntax or runtime template error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub range: Option<Range<usize>>,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TemplateError {}

impl TemplateError {
    pub fn from_minijinja(err: &minijinja::Error, source: &str) -> Self {
        let range = err.range();
        let line = err.line().or_else(|| {
            range.as_ref().map(|r| {
                let prefix = if r.start <= source.len() {
                    &source[..r.start]
                } else {
                    source
                };
                prefix.split('\n').count()
            })
        });
        let column = range.as_ref().and_then(|r| {
            if r.start <= source.len() {
                let prefix = &source[..r.start];
                let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
                Some(source[line_start..r.start].chars().count() + 1)
            } else {
                None
            }
        });
        Self {
            message: err.to_string(),
            line,
            column,
            range,
        }
    }
}

/// Creates a standard MiniJinja environment configured for TickNode templates.
pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_debug(true);
    env.set_fuel(Some(FUEL));
    // `render` replaces this placeholder with a presentation-aware filter.
    env.add_filter("text", |value: Value| value.to_string());
    env.add_global("text", Value::from_iter(Vec::<(String, Value)>::new()));
    env
}

/// Validates template syntax.
pub fn validate(source: &str) -> Result<(), TemplateError> {
    let mut env = environment();
    env.add_template("template", source)
        .map(|_| ())
        .map_err(|err| TemplateError::from_minijinja(&err, source))
}

/// Derives undeclared variable names from the template in order of first appearance.
pub fn variables(source: &str) -> Result<Vec<String>, TemplateError> {
    let mut env = environment();
    env.add_template("template", source)
        .map_err(|err| TemplateError::from_minijinja(&err, source))?;
    let template = env
        .get_template("template")
        .map_err(|err| TemplateError::from_minijinja(&err, source))?;
    let mut names = template.undeclared_variables(false);
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
    names.retain(|name| !env.globals().any(|(global, _)| global == name));
    let mut ordered = Vec::new();
    let mut attribute = false;
    let mut tokens = tokenize(
        source,
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
    Ok(ordered)
}

/// Formats values computed inside a template for the `| text` filter.
pub type RuntimeText = Arc<dyn Fn(&DataValue) -> String + Send + Sync>;

/// A parsed template kept between renders, so a pure node does not reparse on every evaluation.
pub struct CompiledTemplate {
    env: Environment<'static>,
}

impl CompiledTemplate {
    pub fn new(source: &str) -> Result<Self, TemplateError> {
        let mut env = environment();
        env.add_template_owned("template", source.to_string())
            .map_err(|err| TemplateError::from_minijinja(&err, source))?;
        Ok(Self { env })
    }

    /// Renders with the fuel budget, input context, the `text` presentation map and `| text` filter.
    pub fn render(
        &self,
        inputs: &[(String, Option<DataValue>, DataType)],
        format: &dyn Fn(&DataValue, &DataType) -> String,
        runtime_text: RuntimeText,
    ) -> Result<String, TemplateError> {
        let mut env = self.env.clone();
        let text_map: Value = inputs
            .iter()
            .filter_map(|(name, value, ty)| Some((name.clone(), Value::from(format(value.as_ref()?, ty)))))
            .collect();
        env.add_global("text", text_map);
        let zero = runtime_text(&DataValue::Number(crate::NumberValue::Int(0)));
        let int_decimal_suffix = zero.strip_prefix('0').unwrap_or("").to_string();
        env.add_filter("text", move |value: Value| {
            values::format_runtime_value(&value, &*runtime_text, &int_decimal_suffix)
        });
        let context: Value = inputs
            .iter()
            .map(|(name, value, _)| (name.clone(), value.as_ref().map(values::plain).unwrap_or(Value::UNDEFINED)))
            .collect();
        let template = env.get_template("template").expect("compiled template is registered");
        let source = template.source().to_string();
        template.render(context).map_err(|err| TemplateError::from_minijinja(&err, &source))
    }
}

/// Parses and renders once; prefer [`CompiledTemplate`] for repeated renders.
pub fn render(
    source: &str,
    inputs: &[(String, Option<DataValue>, DataType)],
    format: &dyn Fn(&DataValue, &DataType) -> String,
    runtime_text: RuntimeText,
) -> Result<String, TemplateError> {
    CompiledTemplate::new(source)?.render(inputs, format, runtime_text)
}
