use crate::{DataType, TypeKind, TypeRegistry};
use super::catalog;
use super::{filter_and_dedup, type_label, CompletionItem, CompletionKind};

#[derive(Debug, Clone)]
pub(crate) struct ForScope {
    pub vars: Vec<String>,
    pub var_type: Option<DataType>,
}

pub(crate) enum ScanState {
    Plain,
    Comment,
    Interpolation { expr_start: usize, in_str: Option<u8> },
    Tag { expr_start: usize, in_str: Option<u8> },
}

pub(crate) fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

pub(crate) fn scan_prefix(
    source: &str,
    caret: usize,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
) -> (ScanState, Vec<ForScope>, Vec<String>) {
    let prefix = &source[..caret];
    let bytes = prefix.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut state = ScanState::Plain;
    let mut for_stack: Vec<ForScope> = Vec::new();
    let mut set_vars: Vec<String> = Vec::new();

    while i < len {
        match state {
            ScanState::Plain => {
                if bytes[i..].starts_with(b"{#") {
                    i += 2;
                    if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
                        i += 1;
                    }
                    state = ScanState::Comment;
                } else if bytes[i..].starts_with(b"{{") {
                    i += 2;
                    if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
                        i += 1;
                    }
                    state = ScanState::Interpolation { expr_start: i, in_str: None };
                } else if bytes[i..].starts_with(b"{%") {
                    i += 2;
                    if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
                        i += 1;
                    }
                    state = ScanState::Tag { expr_start: i, in_str: None };
                } else {
                    i += 1;
                }
            }
            ScanState::Comment => {
                if bytes[i..].starts_with(b"-#}") || bytes[i..].starts_with(b"+#}") {
                    i += 3;
                    state = ScanState::Plain;
                } else if bytes[i..].starts_with(b"#}") {
                    i += 2;
                    state = ScanState::Plain;
                } else {
                    i += 1;
                }
            }
            ScanState::Interpolation { expr_start, in_str } => {
                if let Some(quote) = in_str {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == quote {
                        i += 1;
                        state = ScanState::Interpolation { expr_start, in_str: None };
                    } else {
                        i += 1;
                    }
                } else if bytes[i] == b'\'' || bytes[i] == b'"' {
                    let quote = bytes[i];
                    i += 1;
                    state = ScanState::Interpolation { expr_start, in_str: Some(quote) };
                } else if bytes[i..].starts_with(b"-}}") || bytes[i..].starts_with(b"+}}") {
                    i += 3;
                    state = ScanState::Plain;
                } else if bytes[i..].starts_with(b"}}") {
                    i += 2;
                    state = ScanState::Plain;
                } else {
                    i += 1;
                }
            }
            ScanState::Tag { expr_start, in_str } => {
                if let Some(quote) = in_str {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else if bytes[i] == quote {
                        i += 1;
                        state = ScanState::Tag { expr_start, in_str: None };
                    } else {
                        i += 1;
                    }
                } else if bytes[i] == b'\'' || bytes[i] == b'"' {
                    let quote = bytes[i];
                    i += 1;
                    state = ScanState::Tag { expr_start, in_str: Some(quote) };
                } else if bytes[i..].starts_with(b"-%}") || bytes[i..].starts_with(b"+%}") {
                    let tag_text = &source[expr_start..i];
                    process_closed_tag(tag_text, inputs, types, &mut for_stack, &mut set_vars);
                    i += 3;
                    state = ScanState::Plain;
                } else if bytes[i..].starts_with(b"%}") {
                    let tag_text = &source[expr_start..i];
                    process_closed_tag(tag_text, inputs, types, &mut for_stack, &mut set_vars);
                    i += 2;
                    state = ScanState::Plain;
                } else {
                    i += 1;
                }
            }
        }
    }

    (state, for_stack, set_vars)
}

pub(crate) fn strip_is_or_is_not(before: &str) -> Option<&str> {
    let s = before.trim_end();
    if let Some(rest) = s.strip_suffix("not")
        && (rest.is_empty() || rest.ends_with(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && c != '_')))
    {
        let rest2 = rest.trim_end();
        if let Some(rest3) = rest2.strip_suffix("is")
            && (rest3.is_empty() || rest3.ends_with(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && c != '_')))
        {
            return Some(rest3);
        }
    }
    if let Some(rest) = s.strip_suffix("is")
        && (rest.is_empty() || rest.ends_with(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && c != '_')))
    {
        return Some(rest);
    }
    None
}

pub(crate) fn is_for_in_iterator(before_slice: &str) -> bool {
    let s = before_slice.trim_start().trim_start_matches(['-', '+']).trim_start();
    let Some(rest) = s.strip_prefix("for") else {
        return false;
    };
    if !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let rest = rest.trim_start();
    for (idx, _) in rest.match_indices("in") {
        let before = &rest[..idx];
        let after_idx = idx + 2;
        let after = &rest[after_idx..];
        let before_ok = before.ends_with(char::is_whitespace) || before.ends_with(')');
        if before_ok && after.chars().all(char::is_whitespace) {
            return true;
        }
    }
    false
}

pub(crate) fn parse_dotted_path(mut s: &str) -> Option<Vec<String>> {
    let mut segments = Vec::new();
    loop {
        s = s.trim_end();
        if s.is_empty() {
            if segments.is_empty() {
                return None;
            }
            break;
        }
        let bytes = s.as_bytes();
        let mut count = 0;
        while count < bytes.len() && is_ident_char(bytes[bytes.len() - 1 - count]) {
            count += 1;
        }
        if count == 0 {
            return None;
        }
        let ident = &s[s.len() - count..];
        segments.push(ident.to_string());
        s = s[..s.len() - count].trim_end();
        if s.ends_with('.') {
            s = &s[..s.len() - 1];
        } else {
            break;
        }
    }
    segments.reverse();
    Some(segments)
}

pub(crate) fn resolve_struct_field(current_type: &DataType, field_name: &str, types: &TypeRegistry) -> Option<DataType> {
    match current_type {
        DataType::Named(name) => {
            let def = types.get(name)?;
            match &def.kind {
                TypeKind::Struct { fields } => {
                    fields.iter().find(|f| f.name == field_name).map(|f| f.data_type.clone())
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn resolve_dot_context(
    before: &str,
    partial: &str,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
    for_stack: &[ForScope],
) -> Vec<CompletionItem> {
    let before_dot = &before[..before.len() - 1];
    let Some(path) = parse_dotted_path(before_dot) else {
        return Vec::new();
    };
    if path.is_empty() {
        return Vec::new();
    }
    let root = &path[0];
    let fields = &path[1..];

    if root == "text" {
        if fields.is_empty() {
            let mut items: Vec<_> = inputs
                .iter()
                .map(|(name, data_type)| CompletionItem {
                    label: name.clone(),
                    kind: CompletionKind::Variable,
                    detail: type_label(data_type, types),
                    insert: name.clone(),
                })
                .collect();
            items.sort_by(|a, b| a.label.cmp(&b.label));
            return filter_and_dedup(items, partial);
        }
        return Vec::new();
    }

    if root == "loop" {
        if !for_stack.is_empty() && fields.is_empty() {
            return filter_and_dedup(catalog::loop_attribute_items(), partial);
        }
        return Vec::new();
    }

    let root_type = if let Some(scope) = for_stack.iter().rev().find(|s| s.vars.iter().any(|v| v == root)) {
        scope.var_type.clone()
    } else if let Some((_, ty)) = inputs.iter().find(|(name, _)| name == root) {
        Some(ty.clone())
    } else {
        None
    };

    let Some(mut current_type) = root_type else {
        return Vec::new();
    };

    for field in fields {
        if let Some(next) = resolve_struct_field(&current_type, field, types) {
            current_type = next;
        } else {
            return Vec::new();
        }
    }

    match current_type {
        DataType::Named(name) => {
            if let Some(def) = types.get(&name)
                && let TypeKind::Struct { fields } = &def.kind
            {
                let mut items: Vec<_> = fields
                    .iter()
                    .map(|f| CompletionItem {
                        label: f.name.clone(),
                        kind: CompletionKind::Field,
                        detail: type_label(&f.data_type, types),
                        insert: f.name.clone(),
                    })
                    .collect();
                items.sort_by(|a, b| a.label.cmp(&b.label));
                return filter_and_dedup(items, partial);
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

fn resolve_iter_type(
    iter_str: &str,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
    for_stack: &[ForScope],
) -> Option<DataType> {
    let path = parse_dotted_path(iter_str)?;
    let root = &path[0];
    let fields = &path[1..];

    let mut current_type = if let Some((_, ty)) = inputs.iter().find(|(name, _)| name == root) {
        ty.clone()
    } else {
        let for_scope = for_stack.iter().rev().find(|s| s.vars.iter().any(|v| v == root))?;
        for_scope.var_type.clone()?
    };

    for field in fields {
        current_type = resolve_struct_field(&current_type, field, types)?;
    }

    if let DataType::List(elem_ty) = current_type {
        Some(*elem_ty)
    } else {
        None
    }
}

fn parse_for_tag(
    s: &str,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
    for_stack: &[ForScope],
) -> Option<ForScope> {
    let rest = s.strip_prefix("for")?.trim_start();
    let mut in_pos = None;
    for (idx, _) in rest.match_indices("in") {
        let before_ok = idx == 0 || rest[..idx].ends_with(char::is_whitespace) || rest[..idx].ends_with(')');
        let after_idx = idx + 2;
        let after_ok = after_idx == rest.len() || rest[after_idx..].starts_with(char::is_whitespace) || rest[after_idx..].starts_with('(');
        if before_ok && after_ok {
            in_pos = Some(idx);
            break;
        }
    }
    let in_pos = in_pos?;
    let targets_str = rest[..in_pos].trim();
    let iter_str = rest[in_pos + 2..].trim();

    let clean_targets = targets_str.trim_matches(|c| c == '(' || c == ')');
    let vars: Vec<String> = clean_targets
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty() && part.bytes().all(is_ident_char))
        .map(str::to_string)
        .collect();
    if vars.is_empty() {
        return None;
    }

    let var_type = if vars.len() == 1 {
        resolve_iter_type(iter_str, inputs, types, for_stack)
    } else {
        None
    };

    Some(ForScope { vars, var_type })
}

fn parse_set_tag(s: &str) -> Option<String> {
    let rest = s.strip_prefix("set")?.trim_start();
    let end = rest.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(rest.len());
    let ident = &rest[..end];
    if !ident.is_empty() && ident.bytes().all(is_ident_char) {
        Some(ident.to_string())
    } else {
        None
    }
}

fn process_closed_tag(
    tag_text: &str,
    inputs: &[(String, DataType)],
    types: &TypeRegistry,
    for_stack: &mut Vec<ForScope>,
    set_vars: &mut Vec<String>,
) {
    let s = tag_text.trim().trim_start_matches(['-', '+']).trim();
    if s.starts_with("for ") || s == "for" {
        // Push a scope even for an unparsable header so the matching `endfor` stays balanced.
        let scope = parse_for_tag(s, inputs, types, for_stack).unwrap_or(ForScope { vars: Vec::new(), var_type: None });
        for_stack.push(scope);
    } else if s.starts_with("endfor") {
        for_stack.pop();
    } else if (s.starts_with("set ") || s == "set")
        && let Some(var) = parse_set_tag(s)
    {
        set_vars.push(var);
    }
}
