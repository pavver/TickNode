use crate::quantity::QuantityLabels;
use crate::{DataType, TypeRegistry};
use std::collections::HashSet;
use std::ops::Range;

pub(crate) mod catalog;
#[cfg(test)]
mod context_tests;
mod date_format;
#[cfg(test)]
mod date_tests;
pub mod docs;
pub(crate) mod infer;
pub mod kinds;
pub(crate) mod scan;
pub mod signature;
pub mod snippets;
#[cfg(test)]
mod tests;
pub(crate) mod tree;

pub use docs::{Doc, DocKind, find_doc};
pub use kinds::{ValueKind, kind_of};
pub use signature::{ParamHelp, SignatureHelp, signature_at};

pub struct CompletionInput<'a> {
    pub source: &'a str,
    /// Byte offset of the caret in `source` (always on a char boundary; clamp if larger than len).
    pub caret: usize,
    /// Template inputs: name + declared/connected type (Any if unknown), in pin order.
    pub inputs: &'a [(String, DataType)],
    pub types: &'a TypeRegistry,
    pub now: Option<chrono::DateTime<chrono::Local>>,
    pub labels: Option<&'a dyn QuantityLabels>,
    pub variables: &'a [crate::Variable],
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
    Format,
    Constant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub icon: Option<String>,
    pub detail: String,
    pub insert: String,
    pub help: Option<crate::UiMessage>,
    pub caret: Option<usize>,
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
        DataType::String => "Text".to_string(),
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

/// Localized TickNode type title, including list element contracts and quantity unit kind.
pub fn localized_type_label(ty: &DataType, input: &CompletionInput) -> String {
    if let DataType::List(inner) = ty {
        let inner = localized_type_label(inner, input);
        return input
            .labels
            .and_then(|l| l.message_args("type-list-of", &[("inner", inner.clone())]))
            .unwrap_or_else(|| format!("List: {inner}"));
    }
    let key = match ty {
        DataType::Named(name) => format!("type-{}-title", crate::fluent_slug(name)),
        _ => format!("type-{}", crate::data_type_name(ty)),
    };
    let title = input
        .labels
        .and_then(|l| l.message(&key))
        .unwrap_or_else(|| type_label(ty, input.types));
    if let DataType::Named(name) = ty
        && let Some(kind) = input
            .types
            .get(name)
            .and_then(|def| def.quantity())
            .or_else(|| crate::quantity::kind(name))
        && let Some(unit) = kind.units.iter().find(|u| u.id == kind.canonical)
    {
        return format!("{title} ({})", unit.symbol);
    }
    title
}

pub fn complete(input: &CompletionInput) -> Option<Completion> {
    complete_with_values(input, &[])
}

/// Completes runtime object fields while retaining declared structure contracts.
pub fn complete_with_values(
    input: &CompletionInput,
    values: &[(String, Option<crate::DataValue>, DataType)],
) -> Option<Completion> {
    complete_with_catalog(input, values, None)
}

/// Completes a slim namespace catalog and cached field trees without expanding other entries.
pub fn complete_with_catalog(
    input: &CompletionInput,
    values: &[(String, Option<crate::DataValue>, DataType)],
    catalog: Option<&crate::plugin_values::editor::ValueTreeContext<'_>>,
) -> Option<Completion> {
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
        scan::ScanState::Interpolation {
            expr_start,
            in_str: Some((quote, quote_pos)),
        }
        | scan::ScanState::Tag {
            expr_start,
            in_str: Some((quote, quote_pos)),
        } => {
            return date_format::try_complete_date_pattern(
                input, expr_start, quote, quote_pos, caret,
            );
        }
        scan::ScanState::Interpolation {
            expr_start,
            in_str: None,
        } => (expr_start, false),
        scan::ScanState::Tag {
            expr_start,
            in_str: None,
        } => (expr_start, true),
    };

    let before_slice = &input.source[expr_start..start];

    // Context 4: right after `{%` (first word of a tag)
    if is_tag && before_slice.chars().all(char::is_whitespace) {
        let after_caret = &input.source[caret..];
        let is_closed = snippets::is_tag_closed_after(after_caret);
        let items = filter_and_dedup(catalog::tag_keyword_items(is_closed), partial);
        return Some(Completion { range, items });
    }

    let before = before_slice.trim_end();

    // Context 1: after `.`
    if before.ends_with('.') {
        if let Some(items) = catalog.and_then(|ctx| {
            let path = scan::parse_dotted_path(before.trim_end_matches('.'))?;
            let root = path.first()?;
            if for_stack.iter().any(|scope| scope.vars.contains(root)) || set_vars.contains(root) {
                return None;
            }
            tree::namespace_items(before, partial, input, ctx)
        }) {
            return Some(Completion { range, items });
        }
        if let Some(items) = tree::dot_items(before, partial, input, values) {
            return Some(Completion { range, items });
        }
        let mut items = scan::resolve_dot_context(
            before,
            partial,
            input.inputs,
            input.types,
            &for_stack,
            input.variables,
        );
        if before.trim() == "text."
            && let Some(ctx) = catalog
        {
            items.extend(tree::namespace_roots(ctx, partial));
        }
        return Some(Completion { range, items });
    }

    // Context 2: after `|`
    if before.ends_with('|') && !before.ends_with("||") {
        let left_text = &before[..before.len() - 1];
        let ctx = infer::InferContext {
            inputs: input.inputs,
            types: input.types,
            for_stack: &for_stack,
            set_vars: &set_vars,
        };
        let left_kind = infer::infer_left(left_text, &ctx);
        let items = filter_and_dedup(catalog::filter_items_for(&left_kind, input.types), partial);
        return Some(Completion { range, items });
    }

    // Context 3: after `is` / `is not`
    if let Some(left_text) = scan::strip_is_or_is_not(before) {
        let ctx = infer::InferContext {
            inputs: input.inputs,
            types: input.types,
            for_stack: &for_stack,
            set_vars: &set_vars,
        };
        let left_kind = infer::infer_left(left_text, &ctx);
        let items = filter_and_dedup(catalog::test_items_for(&left_kind), partial);
        return Some(Completion { range, items });
    }

    // Context 5: any other expression position
    let for_in = is_tag && scan::is_for_in_iterator(before_slice);
    let mut raw_items = Vec::new();

    // 1. Inputs
    for (name, data_type) in input.inputs {
        let kind = kinds::kind_of(data_type, input.types);
        if !for_in || kind.is_iterable() {
            raw_items.push(CompletionItem {
                label: name.clone(),
                icon: None,
                kind: CompletionKind::Variable,
                detail: type_label(data_type, input.types),
                insert: name.clone(),
                help: None,
                caret: None,
            });
        }
    }

    // 2. Open for loop variables
    for scope in for_stack.iter().rev() {
        let is_iter = scope
            .var_type
            .as_ref()
            .map(|t| kinds::kind_of(t, input.types).is_iterable())
            .unwrap_or(true);
        if !for_in || is_iter {
            let detail = scope
                .var_type
                .as_ref()
                .map(|t| type_label(t, input.types))
                .unwrap_or_default();
            for var in &scope.vars {
                raw_items.push(CompletionItem {
                    label: var.clone(),
                    icon: None,
                    kind: CompletionKind::Variable,
                    detail: detail.clone(),
                    insert: var.clone(),
                    help: None,
                    caret: None,
                });
            }
        }
    }

    // 3. Set variables
    for var in &set_vars {
        raw_items.push(CompletionItem {
            label: var.clone(),
            icon: None,
            kind: CompletionKind::Variable,
            detail: String::new(),
            insert: var.clone(),
            help: None,
            caret: None,
        });
    }

    // 4. Loop (only inside an open for, and not right after `for ... in`)
    if !for_stack.is_empty() && !for_in {
        raw_items.push(CompletionItem {
            label: "loop".to_string(),
            icon: None,
            kind: CompletionKind::Variable,
            detail: "loop context".to_string(),
            insert: "loop".to_string(),
            help: None,
            caret: None,
        });
    }

    // 5. Text (not right after `for ... in`)
    if !for_in {
        raw_items.push(CompletionItem {
            label: "text".to_string(),
            icon: None,
            kind: CompletionKind::Variable,
            detail: "formatted inputs".to_string(),
            insert: "text".to_string(),
            help: None,
            caret: None,
        });
        for scope_name in ["var", "sheet", "global"] {
            raw_items.push(CompletionItem {
                label: scope_name.to_string(),
                icon: None,
                kind: CompletionKind::Variable,
                detail: format!("{scope_name} variables"),
                insert: format!("{scope_name}."),
                help: None,
                caret: None,
            });
        }
    }

    if let Some(ctx) = catalog {
        raw_items.extend(tree::namespace_roots(ctx, partial));
    }

    // Functions
    if for_in {
        let help = docs::find_doc(docs::DocKind::Function, "range").map(|d| d.help());
        raw_items.push(CompletionItem {
            label: "range".to_string(),
            icon: None,
            kind: CompletionKind::Function,
            detail: "range([start], stop, [step])".to_string(),
            insert: "range()".to_string(),
            help,
            caret: Some(6),
        });
    } else {
        raw_items.extend(catalog::function_items());
    }

    // Keyword operators (not right after `for ... in`)
    if !for_in {
        raw_items.extend(catalog::keyword_operator_items());
    }

    let items = filter_and_dedup(raw_items, partial);
    Some(Completion { range, items })
}

/// Returns true if the byte offset is inside an interpolation `{{ ... }}` or tag `{% ... %}`,
/// and not inside a comment `{# ... #}` or a string literal.
pub fn expression_at(source: &str, byte: usize) -> bool {
    let mut b = byte.min(source.len());
    while b > 0 && !source.is_char_boundary(b) {
        b -= 1;
    }
    let dummy_types = TypeRegistry::default();
    let (state, _, _) = scan::scan_prefix(source, b, &[], &dummy_types);
    matches!(
        state,
        scan::ScanState::Interpolation { in_str: None, .. }
            | scan::ScanState::Tag { in_str: None, .. }
    )
}

pub(crate) fn filter_and_dedup(items: Vec<CompletionItem>, partial: &str) -> Vec<CompletionItem> {
    let partial_lower = partial.to_ascii_lowercase();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for item in items {
        if item.label.to_ascii_lowercase().starts_with(&partial_lower)
            && seen.insert(item.label.clone())
        {
            result.push(item);
        }
    }
    result
}

pub(crate) fn variable_items(
    scope: &str,
    variables: &[crate::Variable],
    types: &TypeRegistry,
) -> Vec<CompletionItem> {
    let mut map = std::collections::BTreeMap::new();
    for v in variables {
        match scope {
            "sheet" if v.key.scope == crate::VariableScope::Sheet => {
                map.insert(v.key.name.clone(), v.data_type.clone());
            }
            "global" if v.key.scope != crate::VariableScope::Sheet => {
                map.insert(v.key.name.clone(), v.data_type.clone());
            }
            "var"
                if v.key.scope == crate::VariableScope::Sheet || !map.contains_key(&v.key.name) =>
            {
                map.insert(v.key.name.clone(), v.data_type.clone());
            }
            _ => {}
        }
    }
    map.into_iter()
        .map(|(name, ty)| CompletionItem {
            label: name.clone(),
            icon: None,
            kind: CompletionKind::Variable,
            detail: type_label(&ty, types),
            insert: name,
            help: None,
            caret: None,
        })
        .collect()
}

pub(crate) fn find_var_type(
    scope: &str,
    name: &str,
    variables: &[crate::Variable],
) -> Option<DataType> {
    let mut found = None;
    for v in variables {
        if v.key.name != name {
            continue;
        }
        match scope {
            "sheet" if v.key.scope == crate::VariableScope::Sheet => {
                return Some(v.data_type.clone());
            }
            "global" if v.key.scope != crate::VariableScope::Sheet => {
                return Some(v.data_type.clone());
            }
            "var" => {
                if v.key.scope == crate::VariableScope::Sheet {
                    return Some(v.data_type.clone());
                }
                if found.is_none() {
                    found = Some(v.data_type.clone());
                }
            }
            _ => {}
        }
    }
    found
}
