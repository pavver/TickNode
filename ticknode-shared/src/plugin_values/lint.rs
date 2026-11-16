//! Namespace diagnostics share slim catalog lookup and effective field contracts with completion.
use super::editor::ValueTreeContext;
use crate::{
    TypeRegistry, UiMessage,
    template::lint::{Lint, LintSeverity},
};
use std::ops::Range;

/// Static dotted references, excluding strings and template text/comments.
/// Incomplete templates retain references before the tokenizer's first error.
pub fn paths(source: &str, template: bool) -> Vec<(String, Range<usize>)> {
    if template {
        use minijinja::machinery::{Token, tokenize};
        let tokens = tokenize(source, false, Default::default(), Default::default())
            .map_while(Result::ok)
            .collect::<Vec<_>>();
        let mut result = Vec::new();
        let mut i = 0;
        while i < tokens.len() {
            if let Token::Ident(name) = tokens[i].0 {
                let start = tokens[i].1.start_offset as usize;
                let mut end = tokens[i].1.end_offset as usize;
                let mut path = name.to_string();
                while i + 2 < tokens.len() && matches!(tokens[i + 1].0, Token::Dot) {
                    let segment = match tokens[i + 2].0 {
                        Token::Ident(name) => name.to_string(),
                        Token::Int(index) => index.to_string(),
                        _ => break,
                    };
                    path.push('.');
                    path.push_str(&segment);
                    i += 2;
                    end = tokens[i].1.end_offset as usize;
                }
                if path.contains('.') {
                    result.push((path, start..end));
                }
            }
            i += 1;
        }
        result
    } else {
        let mut result = Vec::new();
        let bytes = source.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.' | b':'))
                {
                    i += 1;
                }
                if source[start..i].contains('.') {
                    result.push((source[start..i].into(), start..i));
                }
            } else {
                i += 1;
            }
        }
        result
    }
}

pub fn lint_paths(
    source: &str,
    template: bool,
    ctx: &ValueTreeContext,
    types: &TypeRegistry,
    inputs: &[(String, crate::DataType)],
) -> Vec<Lint> {
    paths(source, template)
        .into_iter()
        .filter_map(|(path, range)| {
            let path = path.strip_prefix("text.").unwrap_or(&path);
            let root = path.split('.').next()?;
            if template {
                let (_, scopes, locals) = crate::template::complete::scan::scan_prefix(
                    source,
                    range.start,
                    inputs,
                    types,
                );
                if scopes
                    .iter()
                    .any(|scope| scope.vars.iter().any(|name| name == root))
                    || locals.iter().any(|name| name == root)
                {
                    return None;
                }
            }
            if !ctx.catalog.roots.contains_key(root) || inputs.iter().any(|(name, _)| name == root)
            {
                return None;
            }
            if ctx.catalog.branch(path).is_some() {
                return None;
            }
            if let Some((namespace, entry, suffix)) = ctx.catalog.entry(path) {
                // Pending field requests cannot establish that a field is unknown.
                ctx.fields(namespace, &entry.key)?;
                if ctx.field_type(path, types).is_some() {
                    return None;
                }
                Some(Lint {
                    range,
                    message: UiMessage::new(
                        "template-lint-unknown-field",
                        format!("Unknown field '{suffix}' on {}", entry.title),
                    )
                    .arg("field", suffix)
                    .arg("type", &entry.title),
                    severity: LintSeverity::Warning,
                })
            } else {
                Some(Lint {
                    range,
                    message: UiMessage::new(
                        "template-lint-unknown-variable",
                        format!("Unknown variable '{path}'"),
                    )
                    .arg("name", path),
                    severity: LintSeverity::Warning,
                })
            }
        })
        .collect()
}
