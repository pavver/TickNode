//! Pure MiniJinja template analysis, syntax validation and rendering.

use crate::{DataType, DataValue};
use chrono::{DateTime, Utc};
use minijinja::{
    Environment, Value,
    machinery::{Token, tokenize},
};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;

pub mod ago;
pub mod dates;
pub mod duration;
mod formatted;
mod namespace;
pub mod plural;
#[cfg(test)]
mod tests;
pub mod unit;
mod values;

pub use ago::ago_filter;
pub use duration::{duration_filter, format_duration_pattern, format_duration_preset};
pub use unit::unit_filter;

pub mod complete;
pub use complete::{
    Completion, CompletionInput, CompletionItem, CompletionKind, Doc, DocKind, ParamHelp,
    SignatureHelp, complete, expression_at, find_doc, signature_at,
};

pub mod highlight;
pub use highlight::{Highlight, TokenClass, highlight};

pub mod lint;
pub use lint::{Lint, LintSeverity, lint, lint_with_variables};

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
    pub fn from_minijinja_mapped(
        err: &minijinja::Error,
        source: &str,
        insertions: &[(usize, usize)],
    ) -> Self {
        let range = err.range().map(|r| {
            let start = unit::map_offset(r.start, insertions);
            let end = unit::map_offset(r.end, insertions);
            start..end.max(start)
        });
        let line = range
            .as_ref()
            .map(|r| {
                let prefix = if r.start <= source.len() {
                    &source[..r.start]
                } else {
                    source
                };
                prefix.split('\n').count()
            })
            .or_else(|| err.line());
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

    pub fn from_minijinja(err: &minijinja::Error, source: &str) -> Self {
        Self::from_minijinja_mapped(err, source, &[])
    }
}

/// Creates a standard MiniJinja environment configured for TickNode templates.
pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_debug(true);
    env.set_fuel(Some(FUEL));
    // `render` replaces this placeholder with a presentation-aware filter.
    env.add_filter("text", |value: Value| value.to_string());
    env.add_filter("unit", |value: Value| value.to_string());
    env.add_global("text", Value::from_iter(Vec::<(String, Value)>::new()));
    env.add_global("var", Value::from_iter(Vec::<(String, Value)>::new()));
    env.add_global("sheet", Value::from_iter(Vec::<(String, Value)>::new()));
    env.add_global("global", Value::from_iter(Vec::<(String, Value)>::new()));
    env.add_function("now", || Utc::now().to_rfc3339());
    let canonical = Arc::new(crate::quantity::CanonicalLabels);
    dates::register(&mut env, canonical.clone());
    ago::register(&mut env, canonical.clone(), Utc::now());
    duration::register(&mut env, canonical.clone());
    plural::register(&mut env, canonical, None, String::new());
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
    variables_with_namespaces(source, &[])
}

pub fn variables_with_namespaces(
    source: &str,
    namespaces: &[String],
) -> Result<Vec<String>, TemplateError> {
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
    names.retain(|name| {
        !namespaces.contains(name) && !env.globals().any(|(global, _)| global == name)
    });
    let mut ordered = Vec::new();
    let mut attribute = false;
    let mut tokens = tokenize(source, false, Default::default(), Default::default()).peekable();
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

/// Formats a value using To text logic for templates.
pub type ToTextFn =
    Arc<dyn Fn(&DataValue, &DataType, &crate::to_text::ToTextOptions) -> String + Send + Sync>;

/// Runtime environment for localized template rendering.
#[derive(Clone)]
pub struct RenderEnv {
    pub labels: Arc<dyn crate::quantity::QuantityLabels + Send + Sync>,
    pub now: DateTime<Utc>,
    pub to_text: ToTextFn,
    pub variables: Vec<crate::Variable>,
    pub plugin_values: Vec<(String, Option<DataValue>, DataType)>,
    pub namespaces: Vec<String>,
}

impl RenderEnv {
    pub fn new(
        labels: Arc<dyn crate::quantity::QuantityLabels + Send + Sync>,
        now: DateTime<Utc>,
        to_text: ToTextFn,
    ) -> Self {
        Self {
            labels,
            now,
            to_text,
            variables: Vec::new(),
            plugin_values: Vec::new(),
            namespaces: Vec::new(),
        }
    }

    pub fn with_variables(mut self, variables: Vec<crate::Variable>) -> Self {
        self.variables = variables;
        self
    }
}

impl Default for RenderEnv {
    fn default() -> Self {
        Self {
            labels: Arc::new(crate::quantity::CanonicalLabels),
            now: Utc::now(),
            to_text: Arc::new(|value, declared, options| {
                crate::to_text::to_text(
                    value,
                    declared,
                    options,
                    &crate::DisplayRules::default(),
                    &crate::quantity::CanonicalLabels,
                )
            }),
            variables: Vec::new(),
            plugin_values: Vec::new(),
            namespaces: Vec::new(),
        }
    }
}

/// Options for compiling and rendering a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TemplateOptions {
    pub trim_blocks: bool,
}

/// A parsed template kept between renders, so a pure node does not reparse on every evaluation.
pub struct CompiledTemplate {
    env: Environment<'static>,
    source: String,
    insertions: Vec<(usize, usize)>,
}

impl CompiledTemplate {
    pub fn new(source: &str) -> Result<Self, TemplateError> {
        Self::new_with(source, TemplateOptions::default())
    }

    pub fn new_with(source: &str, options: TemplateOptions) -> Result<Self, TemplateError> {
        validate(source)?;
        let (rewritten, insertions) = unit::rewrite_unit_paths(source);
        let mut env = environment();
        env.set_trim_blocks(options.trim_blocks);
        env.set_lstrip_blocks(options.trim_blocks);
        env.add_template_owned("template", rewritten)
            .map_err(|err| TemplateError::from_minijinja_mapped(&err, source, &insertions))?;
        Ok(Self {
            env,
            source: source.to_string(),
            insertions,
        })
    }

    /// Static paths, with local set/for bindings excluded by MiniJinja.
    pub fn static_paths(&self) -> Vec<String> {
        self.env
            .get_template("template")
            .map(|t| t.undeclared_variables(true).into_iter().collect())
            .unwrap_or_default()
    }

    /// The original template source.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Renders with the fuel budget, input context, the `text` presentation map, and localized date/time filters.
    pub fn render_localized(
        &self,
        inputs: &[(String, Option<DataValue>, DataType)],
        format: &dyn Fn(&DataValue, &DataType) -> String,
        runtime_text: RuntimeText,
        env_ctx: RenderEnv,
        types: &crate::TypeRegistry,
    ) -> Result<String, TemplateError> {
        let mut env = self.env.clone();
        let now_iso = env_ctx.now.to_rfc3339();
        env.add_function("now", move || now_iso.clone());
        dates::register(&mut env, env_ctx.labels.clone());
        ago::register(&mut env, env_ctx.labels.clone(), env_ctx.now);
        duration::register(&mut env, env_ctx.labels.clone());

        let mut global_map = std::collections::BTreeMap::new();
        let mut sheet_map = std::collections::BTreeMap::new();
        let mut var_map = std::collections::BTreeMap::new();

        let mut text_global_map = std::collections::BTreeMap::new();
        let mut text_sheet_map = std::collections::BTreeMap::new();
        let mut text_var_map = std::collections::BTreeMap::new();

        let (path_types, path_values) = unit::render_path_maps(&env_ctx, inputs, types);
        for v in &env_ctx.variables {
            let plain_val = values::plain(&v.value);
            let scope = if v.key.scope == crate::VariableScope::Sheet {
                "sheet"
            } else {
                "global"
            };
            let text_val = formatted::value(
                &v.value,
                &format!("{scope}.{}", v.key.name),
                &path_types,
                types,
                format,
            );
            match v.key.scope {
                crate::VariableScope::Sheet => {
                    sheet_map.insert(v.key.name.clone(), plain_val.clone());
                    text_sheet_map.insert(v.key.name.clone(), text_val.clone());
                    var_map.insert(v.key.name.clone(), plain_val);
                    text_var_map.insert(v.key.name.clone(), text_val);
                }
                crate::VariableScope::Global | crate::VariableScope::Persistent => {
                    global_map.insert(v.key.name.clone(), plain_val.clone());
                    text_global_map.insert(v.key.name.clone(), text_val.clone());
                    var_map.entry(v.key.name.clone()).or_insert(plain_val);
                    text_var_map.entry(v.key.name.clone()).or_insert(text_val);
                }
            }
        }

        env.add_global("var", Value::from_iter(var_map));
        env.add_global("sheet", Value::from_iter(sheet_map));
        env.add_global("global", Value::from_iter(global_map));

        let mut text_fields: Vec<(String, Value)> = inputs
            .iter()
            .filter_map(|(name, value, _)| {
                Some((
                    name.clone(),
                    formatted::value(value.as_ref()?, name, &path_types, types, format),
                ))
            })
            .collect();
        text_fields.push(("var".to_string(), Value::from_iter(text_var_map)));
        text_fields.push(("sheet".to_string(), Value::from_iter(text_sheet_map)));
        text_fields.push(("global".to_string(), Value::from_iter(text_global_map)));
        namespace::register(
            &mut env,
            &env_ctx,
            &mut text_fields,
            &path_types,
            types,
            format,
        );
        env.add_global("text", Value::from_iter(text_fields));
        unit::register(&mut env, &env_ctx, path_types, path_values);
        let zero = runtime_text(&DataValue::Number(crate::NumberValue::Int(0)));
        let int_decimal_suffix = zero.strip_prefix('0').unwrap_or("").to_string();
        plural::register(
            &mut env,
            env_ctx.labels,
            Some(runtime_text.clone()),
            int_decimal_suffix.clone(),
        );
        env.add_filter("text", move |value: Value| {
            values::format_runtime_value(&value, &*runtime_text, &int_decimal_suffix)
        });
        let context: Value = inputs
            .iter()
            .map(|(name, value, _)| {
                (
                    name.clone(),
                    value
                        .as_ref()
                        .map(values::plain)
                        .unwrap_or(Value::UNDEFINED),
                )
            })
            .collect();
        let template = env
            .get_template("template")
            .map_err(|err| TemplateError::from_minijinja(&err, &self.source))?;
        template.render(context).map_err(|err| {
            TemplateError::from_minijinja_mapped(&err, &self.source, &self.insertions)
        })
    }

    /// Renders with canonical (English/ISO) labels; prefer [`Self::render_localized`] for UI/locale-specific rendering.
    pub fn render(
        &self,
        inputs: &[(String, Option<DataValue>, DataType)],
        format: &dyn Fn(&DataValue, &DataType) -> String,
        runtime_text: RuntimeText,
    ) -> Result<String, TemplateError> {
        let dummy_types = crate::TypeRegistry::new();
        self.render_localized(
            inputs,
            format,
            runtime_text,
            RenderEnv::default(),
            &dummy_types,
        )
    }
}

/// Parses and renders once with canonical labels; prefer [`CompiledTemplate::render_localized`] for repeated or localized renders.
pub fn render(
    source: &str,
    inputs: &[(String, Option<DataValue>, DataType)],
    format: &dyn Fn(&DataValue, &DataType) -> String,
    runtime_text: RuntimeText,
) -> Result<String, TemplateError> {
    CompiledTemplate::new(source)?.render(inputs, format, runtime_text)
}
