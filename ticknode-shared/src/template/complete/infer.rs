use super::catalog;
use super::kinds::{self, ValueKind};
use super::scan::{self, ForScope};
use crate::{DataType, TypeRegistry};

pub(crate) struct InferContext<'a> {
    pub inputs: &'a [(String, DataType)],
    pub types: &'a TypeRegistry,
    pub for_stack: &'a [ForScope],
    pub set_vars: &'a [String],
}

pub(crate) fn infer_left(before: &str, ctx: &InferContext) -> ValueKind {
    let s = before.trim();
    if s.is_empty() {
        return ValueKind::Any;
    }

    if let Some((rest, filter_name)) = split_trailing_filter(s) {
        let input_kind = infer_left(rest, ctx);
        return catalog::filter_output_kind(filter_name, &input_kind);
    }

    infer_base_expr(s, ctx)
}

fn split_trailing_filter(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let mut idx = bytes.len();
    while idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
        idx -= 1;
    }
    if idx == 0 {
        return None;
    }

    if bytes[idx - 1] == b')' {
        let mut paren_depth = 0;
        let mut in_str: Option<u8> = None;
        let mut pos = idx;
        while pos > 0 {
            pos -= 1;
            let b = bytes[pos];
            if let Some(quote) = in_str {
                if b == quote {
                    let mut slash_count = 0;
                    let mut p = pos;
                    while p > 0 && bytes[p - 1] == b'\\' {
                        slash_count += 1;
                        p -= 1;
                    }
                    if slash_count % 2 == 0 {
                        in_str = None;
                    }
                }
            } else if b == b'\'' || b == b'"' {
                in_str = Some(b);
            } else if b == b')' {
                paren_depth += 1;
            } else if b == b'(' {
                paren_depth -= 1;
                if paren_depth == 0 {
                    idx = pos;
                    break;
                }
            }
        }
        if paren_depth != 0 || in_str.is_some() {
            return None;
        }
        while idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
            idx -= 1;
        }
    }

    let name_end = idx;
    while idx > 0 && scan::is_ident_char(bytes[idx - 1]) {
        idx -= 1;
    }
    let name_start = idx;
    if name_start == name_end {
        return None;
    }
    let filter_name = &s[name_start..name_end];

    while idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
        idx -= 1;
    }
    if idx == 0 {
        return None;
    }

    if bytes[idx - 1] == b'|' {
        if idx >= 2 && bytes[idx - 2] == b'|' {
            return None;
        }
        let rest = &s[..idx - 1];
        Some((rest, filter_name))
    } else {
        None
    }
}

fn infer_base_expr(s: &str, ctx: &InferContext) -> ValueKind {
    // 1. String literal: ends with quote
    if let Some(&quote) = s.as_bytes().last().filter(|&&b| b == b'\'' || b == b'"') {
        let bytes = s.as_bytes();
        let mut pos = bytes.len() - 1;
        let mut found = false;
        while pos > 0 {
            pos -= 1;
            if bytes[pos] == quote {
                let mut slash_count = 0;
                let mut p = pos;
                while p > 0 && bytes[p - 1] == b'\\' {
                    slash_count += 1;
                    p -= 1;
                }
                if slash_count % 2 == 0 {
                    found = true;
                    break;
                }
            }
        }
        if found {
            return ValueKind::String;
        }
    }

    // 2. Bracket literal: list
    if s.ends_with(']') {
        return ValueKind::List(Box::new(ValueKind::Any));
    }

    // 3. Brace literal: map
    if s.ends_with('}') {
        return ValueKind::Map(None);
    }

    // 4. Parentheses or function call
    if s.ends_with(')') {
        return ValueKind::Any;
    }

    // 5. Boolean literals
    if let Some(rest) = s.strip_suffix("true")
        && (rest.is_empty()
            || rest.ends_with(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && c != '_')))
    {
        return ValueKind::Bool;
    }
    if let Some(rest) = s.strip_suffix("false")
        && (rest.is_empty()
            || rest.ends_with(|c: char| c.is_whitespace() || (!c.is_alphanumeric() && c != '_')))
    {
        return ValueKind::Bool;
    }

    // 6. Number literal
    if let Some(kind) = check_number_literal(s) {
        return kind;
    }

    // 7. Dotted path
    if let Some(path) = scan::parse_dotted_path(s) {
        return resolve_dotted_path_kind(&path, ctx);
    }

    ValueKind::Any
}

fn check_number_literal(s: &str) -> Option<ValueKind> {
    let bytes = s.as_bytes();
    let mut i = bytes.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i == bytes.len() {
        return None;
    }
    if i > 0 && bytes[i - 1] == b'.' && i > 1 && bytes[i - 2].is_ascii_digit() {
        i -= 1;
        while i > 0 && bytes[i - 1].is_ascii_digit() {
            i -= 1;
        }
    }
    if i > 0 && bytes[i - 1] == b'-' {
        i -= 1;
    }
    if i == 0
        || bytes[i - 1].is_ascii_whitespace()
        || (!scan::is_ident_char(bytes[i - 1]) && bytes[i - 1] != b'.')
    {
        return Some(ValueKind::Number);
    }
    None
}

fn resolve_dotted_path_kind(path: &[String], ctx: &InferContext) -> ValueKind {
    if path.is_empty() {
        return ValueKind::Any;
    }
    let root = &path[0];
    let fields = &path[1..];

    if root == "text" {
        if fields.len() == 1 {
            return ValueKind::String;
        }
        return ValueKind::Any;
    }

    if root == "loop" {
        if !ctx.for_stack.is_empty() && fields.len() == 1 {
            return match fields[0].as_str() {
                "index" | "index0" | "revindex" | "revindex0" | "length" | "depth" | "depth0" => {
                    ValueKind::Number
                }
                "first" | "last" | "changed" => ValueKind::Bool,
                "previtem" | "nextitem" => ValueKind::Any,
                _ => ValueKind::Any,
            };
        }
        return ValueKind::Any;
    }

    if ctx.set_vars.iter().any(|v| v == root) {
        return ValueKind::Any;
    }

    let root_type = if let Some(scope) = ctx
        .for_stack
        .iter()
        .rev()
        .find(|s| s.vars.iter().any(|v| v == root))
    {
        scope.var_type.clone()
    } else if let Some((_, ty)) = ctx.inputs.iter().find(|(name, _)| name == root) {
        Some(ty.clone())
    } else {
        None
    };

    let Some(mut current_type) = root_type else {
        return ValueKind::Any;
    };

    for field in fields {
        if let Some(next) = scan::resolve_struct_field(&current_type, field, ctx.types) {
            current_type = next;
        } else {
            return ValueKind::Any;
        }
    }

    kinds::kind_of(&current_type, ctx.types)
}
