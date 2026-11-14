//! Signature help for template filters, tests, and functions.

use super::docs::{DocKind, find_doc, param_help_key};
use crate::UiMessage;

#[cfg(test)]
mod tests;

/// Parameter documentation in signature help.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamHelp {
    pub name: String,
    pub help: UiMessage,
}

impl AsRef<str> for ParamHelp {
    fn as_ref(&self) -> &str {
        &self.name
    }
}

/// Active signature help at a caret position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureHelp {
    pub name: String,
    pub kind: DocKind,
    pub signature: String,
    pub active_param: usize,
    pub params: Vec<ParamHelp>,
    pub help: UiMessage,
}

impl SignatureHelp {
    /// Returns the active parameter description, or the overall item description as fallback.
    pub fn active_param_help(&self) -> &UiMessage {
        self.params
            .get(self.active_param)
            .map(|p| &p.help)
            .unwrap_or(&self.help)
    }
}

#[derive(Clone)]
struct OpenCall {
    doc: &'static super::docs::Doc,
    paren_depth: usize,
    bracket_depth: usize,
    brace_depth: usize,
    open_pos: usize,
    active_arg_idx: usize,
    active_arg_start: usize,
    is_paren: bool,
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn read_ident(bytes: &[u8], mut pos: usize) -> Option<(&str, usize)> {
    let start = pos;
    while pos < bytes.len() && is_ident_char(bytes[pos]) {
        pos += 1;
    }
    if pos > start {
        std::str::from_utf8(&bytes[start..pos])
            .ok()
            .map(|s| (s, pos))
    } else {
        None
    }
}

fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn find_keyword_arg(arg_slice: &str) -> Option<&str> {
    let trimmed = arg_slice.trim_start();
    let bytes = trimmed.as_bytes();
    let (ident, after_ident) = read_ident(bytes, 0)?;
    let after_ws = skip_whitespace(bytes, after_ident);
    if after_ws < bytes.len() && bytes[after_ws] == b'=' {
        if after_ws + 1 < bytes.len() && bytes[after_ws + 1] == b'=' {
            return None;
        }
        Some(ident)
    } else {
        None
    }
}

/// Computes signature help when the caret is inside an argument list of a filter, test, or function.
pub fn signature_at(source: &str, caret: usize) -> Option<SignatureHelp> {
    let mut caret = caret.min(source.len());
    while !source.is_char_boundary(caret) {
        caret -= 1;
    }

    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i..].starts_with(b"{#") {
            let start = i;
            i += 2;
            while i < len {
                if bytes[i..].starts_with(b"#}") {
                    i += 2;
                    break;
                } else if bytes[i..].starts_with(b"-#}") || bytes[i..].starts_with(b"+#}") {
                    i += 3;
                    break;
                }
                i += 1;
            }
            if caret >= start && caret <= i {
                return None;
            }
            continue;
        }

        let is_interp = bytes[i..].starts_with(b"{{");
        let is_tag = bytes[i..].starts_with(b"{%");

        if !is_interp && !is_tag {
            i += 1;
            continue;
        }

        let expr_start = if i + 2 < len && (bytes[i + 2] == b'-' || bytes[i + 2] == b'+') {
            i + 3
        } else {
            i + 2
        };

        let mut pos = expr_start;
        let mut expr_end = len;
        let mut in_quote = None;

        while pos < len {
            if let Some(q) = in_quote {
                if bytes[pos] == b'\\' {
                    pos += 2;
                    continue;
                }
                if bytes[pos] == q {
                    in_quote = None;
                }
                pos += 1;
            } else if bytes[pos] == b'\'' || bytes[pos] == b'"' {
                in_quote = Some(bytes[pos]);
                pos += 1;
            } else if (is_interp
                && (bytes[pos..].starts_with(b"}}")
                    || bytes[pos..].starts_with(b"-}}")
                    || bytes[pos..].starts_with(b"+}}")))
                || (is_tag
                    && (bytes[pos..].starts_with(b"%}")
                        || bytes[pos..].starts_with(b"-%}")
                        || bytes[pos..].starts_with(b"+%}")))
            {
                expr_end = pos;
                break;
            } else {
                pos += 1;
            }
        }

        if caret < expr_start || caret > expr_end {
            i = pos;
            continue;
        }

        return parse_calls_in_expr(source, expr_start, expr_end, caret);
    }

    None
}

fn parse_calls_in_expr(
    source: &str,
    expr_start: usize,
    expr_end: usize,
    caret: usize,
) -> Option<SignatureHelp> {
    let bytes = source.as_bytes();
    let mut pos = expr_start;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;

    let mut call_stack: Vec<OpenCall> = Vec::new();
    let mut last_ident: Option<(&str, usize, usize)> = None;
    let mut after_pipe = false;
    let mut after_is = false;

    while pos < expr_end {
        pos = skip_whitespace(bytes, pos);
        if pos >= expr_end {
            break;
        }

        let b = bytes[pos];

        if b == b'\'' || b == b'"' {
            let quote = b;
            let str_start = pos;
            pos += 1;
            while pos < expr_end {
                if bytes[pos] == b'\\' {
                    pos += 2;
                    continue;
                }
                if bytes[pos] == quote {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            let str_end = pos;

            if caret >= str_start && caret <= str_end && call_stack.is_empty() {
                return None;
            }

            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b'(' {
            let open_paren = pos;
            pos += 1;
            paren_depth += 1;

            let mut call_doc = None;
            if let Some((name, _, _)) = last_ident {
                if after_pipe {
                    call_doc = find_doc(DocKind::Filter, name);
                } else if after_is {
                    call_doc = find_doc(DocKind::Test, name);
                } else {
                    call_doc = find_doc(DocKind::Function, name)
                        .or_else(|| find_doc(DocKind::Filter, name));
                }
            }

            if let Some(doc) = call_doc {
                call_stack.push(OpenCall {
                    doc,
                    paren_depth,
                    bracket_depth,
                    brace_depth,
                    open_pos: open_paren,
                    active_arg_idx: 0,
                    active_arg_start: pos,
                    is_paren: true,
                });
            }

            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b')' {
            if let Some(top) = call_stack.last()
                && top.is_paren
                && top.paren_depth == paren_depth
            {
                if caret > top.open_pos && caret <= pos {
                    return build_signature_help(source, top, pos, caret);
                }
                call_stack.pop();
            }
            paren_depth = paren_depth.saturating_sub(1);
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b'[' {
            bracket_depth += 1;
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b']' {
            bracket_depth = bracket_depth.saturating_sub(1);
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b'{' {
            brace_depth += 1;
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b'}' {
            brace_depth = brace_depth.saturating_sub(1);
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b',' {
            let comma_pos = pos;
            if let Some(top) = call_stack.last_mut()
                && top.paren_depth == paren_depth
                && top.bracket_depth == bracket_depth
                && top.brace_depth == brace_depth
            {
                if caret > top.open_pos && caret <= comma_pos {
                    return build_signature_help(source, top, comma_pos, caret);
                }
                top.active_arg_idx += 1;
                top.active_arg_start = comma_pos + 1;
            }
            pos += 1;
            last_ident = None;
            after_pipe = false;
            after_is = false;
            continue;
        }

        if b == b'|' {
            if let Some(top) = call_stack.last()
                && !top.is_paren
            {
                if caret > top.open_pos && caret <= pos {
                    return build_signature_help(source, top, pos, caret);
                }
                call_stack.pop();
            }
            after_pipe = true;
            after_is = false;
            last_ident = None;
            pos += 1;
            continue;
        }

        if is_ident_char(b) {
            let start = pos;
            if let Some((ident, end)) = read_ident(bytes, start) {
                pos = end;

                if (ident == "and" || ident == "or" || ident == "if" || ident == "else")
                    && let Some(top) = call_stack.last()
                    && !top.is_paren
                {
                    if caret > top.open_pos && caret <= start {
                        return build_signature_help(source, top, start, caret);
                    }
                    call_stack.pop();
                }

                if ident == "is" {
                    after_is = true;
                    after_pipe = false;
                    last_ident = None;

                    let next_pos = skip_whitespace(bytes, pos);
                    if let Some(("not", end_not)) = read_ident(bytes, next_pos) {
                        pos = end_not;
                    }
                    continue;
                }

                if after_is && let Some(doc) = find_doc(DocKind::Test, ident) {
                    let ws_pos = skip_whitespace(bytes, pos);
                    if ws_pos < expr_end && bytes[ws_pos] != b'(' {
                        call_stack.push(OpenCall {
                            doc,
                            paren_depth,
                            bracket_depth,
                            brace_depth,
                            open_pos: pos,
                            active_arg_idx: 0,
                            active_arg_start: ws_pos,
                            is_paren: false,
                        });
                        after_is = false;
                        last_ident = None;
                        continue;
                    }
                }

                last_ident = Some((ident, start, end));
                continue;
            }
        }

        pos += 1;
    }

    if let Some(top) = call_stack.last()
        && caret > top.open_pos
        && caret <= expr_end
    {
        return build_signature_help(source, top, expr_end, caret);
    }

    None
}

fn build_signature_help(
    source: &str,
    call: &OpenCall,
    arg_end: usize,
    caret: usize,
) -> Option<SignatureHelp> {
    let _ = caret;
    if caret <= call.open_pos {
        return None;
    }

    let doc = call.doc;
    let arg_start = call.active_arg_start.min(source.len());
    let end = arg_end.min(source.len());

    let mut active_param = call.active_arg_idx;
    if arg_start < end {
        let arg_slice = &source[arg_start..end];
        if let Some(keyword) = find_keyword_arg(arg_slice)
            && let Some(idx) = doc.params.iter().position(|(name, _)| *name == keyword)
        {
            active_param = idx;
        }
    }

    let params = doc
        .params
        .iter()
        .map(|(param_name, fallback)| {
            let key = param_help_key(doc.kind, doc.name, param_name);
            ParamHelp {
                name: (*param_name).to_string(),
                help: UiMessage::new(key, *fallback),
            }
        })
        .collect();

    Some(SignatureHelp {
        name: doc.name.to_string(),
        kind: doc.kind,
        signature: doc.signature.to_string(),
        active_param,
        params,
        help: doc.help(),
    })
}
