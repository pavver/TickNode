use std::collections::HashSet;
use std::ops::Range;
use crate::{DataType, TypeRegistry};

mod catalog;
mod scan;
#[cfg(test)]
mod tests;

pub struct CompletionInput<'a> {
    pub source: &'a str,
    /// Byte offset of the caret in `source` (always on a char boundary; clamp if larger than len).
    pub caret: usize,
    /// Template inputs: name + declared/connected type (Any if unknown), in pin order.
    pub inputs: &'a [(String, DataType)],
    pub types: &'a TypeRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionKind {
    Variable,
    Field,
    Filter,
    Test,
    Function,
    Keyword,
    Attribute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: String,
    pub insert: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub range: Range<usize>,
    pub items: Vec<CompletionItem>,
}

/// Short English type name for vars/fields.
pub fn type_label(data_type: &DataType, types: &TypeRegistry) -> String {
    match data_type {
        DataType::Bool => "Bool".to_string(),
        DataType::Number => "Number".to_string(),
        DataType::String => "String".to_string(),
        DataType::Time => "Time".to_string(),
        DataType::DateTime => "DateTime".to_string(),
        DataType::Duration => "Duration".to_string(),
        DataType::Color => "Color".to_string(),
        DataType::Object => "Object".to_string(),
        DataType::Any => "Any".to_string(),
        DataType::List(inner) => format!("list of {}", type_label(inner, types)),
        DataType::Named(name) => {
            if let Some(def) = types.get(name) {
                def.title.clone()
            } else {
                name.clone()
            }
        }
    }
}

pub fn complete(input: &CompletionInput) -> Option<Completion> {
    let mut caret = input.caret.min(input.source.len());
    while !input.source.is_char_boundary(caret) {
        caret -= 1;
    }

    let bytes = input.source.as_bytes();
    let mut start = caret;
    while start > 0 && scan::is_ident_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = caret;
    while end < bytes.len() && scan::is_ident_char(bytes[end]) {
        end += 1;
    }
    let range = start..end;
    let partial = &input.source[start..caret];

    let (state, for_stack, set_vars) =
        scan::scan_prefix(input.source, caret, input.inputs, input.types);

    let (expr_start, is_tag) = match state {
        scan::ScanState::Plain | scan::ScanState::Comment => return None,
        scan::ScanState::Interpolation { in_str: Some(_), .. }
        | scan::ScanState::Tag { in_str: Some(_), .. } => return None,
        scan::ScanState::Interpolation { expr_start, in_str: None } => (expr_start, false),
        scan::ScanState::Tag { expr_start, in_str: None } => (expr_start, true),
    };

    let before_slice = &input.source[expr_start..start];

    // Context 4: right after `{%` (first word of a tag)
    if is_tag && before_slice.chars().all(char::is_whitespace) {
        let items = filter_and_dedup(catalog::tag_keyword_items(), partial);
        return Some(Completion { range, items });
    }

    let before = before_slice.trim_end();

    // Context 1: after `.`
    if before.ends_with('.') {
        let items = scan::resolve_dot_context(before, partial, input.inputs, input.types, &for_stack);
        return Some(Completion { range, items });
    }

    // Context 2: after `|`
    if before.ends_with('|') && !before.ends_with("||") {
        let items = filter_and_dedup(catalog::filter_items(), partial);
        return Some(Completion { range, items });
    }

    // Context 3: after `is` / `is not`
    if scan::is_after_is_or_is_not(before) {
        let items = filter_and_dedup(catalog::test_items(), partial);
        return Some(Completion { range, items });
    }

    // Context 5: any other expression position
    let mut raw_items = Vec::new();

    // 1. Inputs
    for (name, data_type) in input.inputs {
        raw_items.push(CompletionItem {
            label: name.clone(),
            kind: CompletionKind::Variable,
            detail: type_label(data_type, input.types),
            insert: name.clone(),
        });
    }

    // 2. Open for loop variables
    for scope in for_stack.iter().rev() {
        let detail = scope.var_type.as_ref().map(|t| type_label(t, input.types)).unwrap_or_default();
        for var in &scope.vars {
            raw_items.push(CompletionItem {
                label: var.clone(),
                kind: CompletionKind::Variable,
                detail: detail.clone(),
                insert: var.clone(),
            });
        }
    }

    // 3. Set variables
    for var in &set_vars {
        raw_items.push(CompletionItem {
            label: var.clone(),
            kind: CompletionKind::Variable,
            detail: String::new(),
            insert: var.clone(),
        });
    }

    // 4. Loop (only inside an open for)
    if !for_stack.is_empty() {
        raw_items.push(CompletionItem {
            label: "loop".to_string(),
            kind: CompletionKind::Variable,
            detail: "loop context".to_string(),
            insert: "loop".to_string(),
        });
    }

    // 5. Text
    raw_items.push(CompletionItem {
        label: "text".to_string(),
        kind: CompletionKind::Variable,
        detail: "formatted inputs".to_string(),
        insert: "text".to_string(),
    });

    // Functions
    raw_items.extend(catalog::function_items());

    // Keyword operators
    raw_items.extend(catalog::keyword_operator_items());

    let items = filter_and_dedup(raw_items, partial);
    Some(Completion { range, items })
}

pub(crate) fn filter_and_dedup(items: Vec<CompletionItem>, partial: &str) -> Vec<CompletionItem> {
    let partial_lower = partial.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        if item.label.to_ascii_lowercase().starts_with(&partial_lower) && seen.insert(item.label.clone()) {
            result.push(item);
        }
    }
    result
}
