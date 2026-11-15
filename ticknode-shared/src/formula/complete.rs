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
    b.is_ascii_alphanumeric() || b == b'_' || b == b':'
}

/// Provides completion items (functions, constants, inputs) at `caret` in `source`.
pub fn complete(source: &str, caret: usize, inputs: &[(String, DataType)]) -> Completion {
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

    // 1. Inputs
    for (name, data_type) in inputs {
        if prefix.is_empty() || name.to_ascii_lowercase().starts_with(&prefix_lower) {
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

    // 2. Constants
    for (name, detail, desc) in CONSTANTS {
        if prefix.is_empty() || name.starts_with(&prefix_lower) {
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

    // 3. Functions from catalog (including aliases)
    for doc in CATALOG {
        if prefix.is_empty() || doc.name.to_ascii_lowercase().starts_with(&prefix_lower) {
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
            if prefix.is_empty() || alias.to_ascii_lowercase().starts_with(&prefix_lower) {
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

    Completion {
        range: prefix_start..aligned_caret,
        items,
    }
}
