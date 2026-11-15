//! Completion engine for formula properties.

use super::catalog::CATALOG;
use crate::DataType;
use crate::UiMessage;
pub use crate::template::complete::{Completion, CompletionItem, CompletionKind};

#[cfg(test)]
mod tests;

const CONSTANTS: &[(&str, &str, &str)] = &[
    ("pi", "3.14159...", "Archimedes' constant pi"),
    ("e", "2.71828...", "Euler's number e"),
    ("tau", "6.28318...", "Circle constant tau (2*pi)"),
    ("inf", "Infinity", "Positive infinity"),
    ("nan", "NaN", "Not a number (NaN)"),
    ("true", "Bool", "Boolean true"),
    ("false", "Bool", "Boolean false"),
];

fn is_prefix_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b':' || b == b'.'
}

fn is_numeric_type(ty: &DataType) -> bool {
    matches!(ty, DataType::Number | DataType::Bool)
        || matches!(ty, DataType::Named(name) if crate::quantity::kind(name).is_some())
}

/// Provides completion items (functions, constants, inputs) at `caret` in `source`.
pub fn complete(source: &str, caret: usize, inputs: &[(String, DataType)]) -> Completion {
    complete_with_variables(source, caret, inputs, &[])
}

/// Provides completion items (functions, constants, inputs, variables) at `caret` in `source`.
pub fn complete_with_variables(
    source: &str,
    caret: usize,
    inputs: &[(String, DataType)],
    variables: &[crate::Variable],
) -> Completion {
    let caret = caret.min(source.len());
    let mut aligned_caret = caret;
    while !source.is_char_boundary(aligned_caret) {
        aligned_caret -= 1;
    }

    let bytes = source.as_bytes();
    let mut prefix_start = aligned_caret;
    while prefix_start > 0 && is_prefix_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let prefix = &source[prefix_start..aligned_caret];
    let prefix_lower = prefix.to_ascii_lowercase();

    let mut items = Vec::new();
    collect_inputs(inputs, prefix, &prefix_lower, &mut items);
    collect_variables(variables, prefix, &prefix_lower, &mut items);
    collect_constants(prefix, &prefix_lower, &mut items);
    collect_functions(prefix, &prefix_lower, &mut items);

    Completion {
        range: prefix_start..aligned_caret,
        items,
    }
}

fn collect_inputs(
    inputs: &[(String, DataType)],
    prefix: &str,
    prefix_lower: &str,
    items: &mut Vec<CompletionItem>,
) {
    for (name, data_type) in inputs {
        if prefix.is_empty() || name.to_ascii_lowercase().starts_with(prefix_lower) {
            let detail = crate::template::complete::type_label(data_type, &Default::default());
            items.push(CompletionItem {
                label: name.clone(),
                kind: CompletionKind::Variable,
                detail,
                insert: name.clone(),
                help: None,
                caret: None,
            });
        }
    }
}

fn collect_variables(
    variables: &[crate::Variable],
    prefix: &str,
    prefix_lower: &str,
    items: &mut Vec<CompletionItem>,
) {
    let mut seen_candidates = std::collections::HashSet::new();
    for v in variables {
        if !is_numeric_type(&v.data_type) {
            continue;
        }
        let detail = crate::template::complete::type_label(&v.data_type, &Default::default());
        let mut candidates = vec![format!("var.{}", v.key.name)];
        match v.key.scope {
            crate::VariableScope::Sheet => {
                candidates.push(format!("sheet.{}", v.key.name));
            }
            crate::VariableScope::Global | crate::VariableScope::Persistent => {
                candidates.push(format!("global.{}", v.key.name));
            }
        }
        for cand in candidates {
            if seen_candidates.insert(cand.clone())
                && (prefix.is_empty() || cand.to_ascii_lowercase().starts_with(prefix_lower))
            {
                items.push(CompletionItem {
                    label: cand.clone(),
                    kind: CompletionKind::Variable,
                    detail: detail.clone(),
                    insert: cand,
                    help: None,
                    caret: None,
                });
            }
        }
    }
}

fn collect_constants(prefix: &str, prefix_lower: &str, items: &mut Vec<CompletionItem>) {
    for (name, detail, desc) in CONSTANTS {
        if prefix.is_empty() || name.starts_with(prefix_lower) {
            items.push(CompletionItem {
                label: (*name).to_string(),
                kind: CompletionKind::Constant,
                detail: (*detail).to_string(),
                insert: (*name).to_string(),
                help: Some(UiMessage::new(format!("formula-const-{name}"), *desc)),
                caret: None,
            });
        }
    }
}

fn collect_functions(prefix: &str, prefix_lower: &str, items: &mut Vec<CompletionItem>) {
    for doc in CATALOG {
        if prefix.is_empty() || doc.name.to_ascii_lowercase().starts_with(prefix_lower) {
            items.push(CompletionItem {
                label: doc.name.to_string(),
                kind: CompletionKind::Function,
                detail: doc.signature.to_string(),
                insert: format!("{}(", doc.name),
                help: Some(UiMessage::new(
                    format!("formula-fn-{}", doc.name),
                    doc.description,
                )),
                caret: Some(doc.name.len() + 1),
            });
        }
        for alias in doc.aliases {
            if prefix.is_empty() || alias.to_ascii_lowercase().starts_with(prefix_lower) {
                items.push(CompletionItem {
                    label: (*alias).to_string(),
                    kind: CompletionKind::Function,
                    detail: doc.signature.to_string(),
                    insert: format!("{alias}("),
                    help: Some(UiMessage::new(
                        format!("formula-fn-{}", doc.name),
                        doc.description,
                    )),
                    caret: Some(alias.len() + 1),
                });
            }
        }
    }
}
