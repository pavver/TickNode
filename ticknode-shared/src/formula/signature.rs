//! Active signature help for formula functions.

use super::catalog::find_function;
use crate::UiMessage;
pub use crate::template::complete::{DocKind, ParamHelp, SignatureHelp};

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct OpenCall {
    func_name: String,
    paren_depth: usize,
    active_arg: usize,
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b':'
}

/// Parses parameter names from a function signature string like `clamp(x, lo, hi)` or `round(x[, digits])`.
pub fn parse_signature_params(signature: &str) -> Vec<String> {
    let Some(start_paren) = signature.find('(') else {
        return Vec::new();
    };
    let Some(end_paren) = signature.rfind(')') else {
        return Vec::new();
    };
    if end_paren <= start_paren + 1 {
        return Vec::new();
    }
    let inner = &signature[start_paren + 1..end_paren];
    inner
        .split(',')
        .map(|part| {
            part.chars()
                .filter(|&c| c != '[' && c != ']')
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

/// Computes active function signature help at `caret` in `source`.
pub fn signature_at(source: &str, caret: usize) -> Option<SignatureHelp> {
    let caret = caret.min(source.len());
    let mut aligned_caret = caret;
    while !source.is_char_boundary(aligned_caret) {
        aligned_caret -= 1;
    }

    let bytes = &source.as_bytes()[..aligned_caret];
    let len = bytes.len();
    let mut i = 0;
    let mut paren_depth = 0;
    let mut call_stack: Vec<OpenCall> = Vec::new();

    while i < len {
        let b = bytes[i];

        if is_ident_char(b) {
            let start = i;
            while i < len && is_ident_char(bytes[i]) {
                i += 1;
            }
            let ident = &source[start..i];

            // Check if immediately followed by '(' (ignoring whitespace)
            let mut next_k = i;
            while next_k < len && bytes[next_k].is_ascii_whitespace() {
                next_k += 1;
            }
            if next_k < len && bytes[next_k] == b'(' {
                i = next_k + 1;
                paren_depth += 1;
                call_stack.push(OpenCall {
                    func_name: ident.to_string(),
                    paren_depth,
                    active_arg: 0,
                });
                continue;
            }
            continue;
        }

        if b == b'(' {
            paren_depth += 1;
            i += 1;
            continue;
        }

        if b == b')' {
            if let Some(top) = call_stack.last()
                && top.paren_depth == paren_depth
            {
                call_stack.pop();
            }
            paren_depth = paren_depth.saturating_sub(1);
            i += 1;
            continue;
        }

        if b == b',' {
            if let Some(top) = call_stack.last_mut()
                && top.paren_depth == paren_depth
            {
                top.active_arg += 1;
            }
            i += 1;
            continue;
        }

        i += 1;
    }

    let top = call_stack.last()?;
    let doc = find_function(&top.func_name)?;
    let raw_params = parse_signature_params(doc.signature);

    let params: Vec<ParamHelp> = raw_params
        .iter()
        .map(|p| ParamHelp {
            name: p.clone(),
            help: UiMessage::new(format!("formula-fn-{}-{p}", doc.name), doc.description),
        })
        .collect();

    let active_param = if params.is_empty() {
        0
    } else if top.active_arg < params.len() {
        top.active_arg
    } else {
        params.len().saturating_sub(1)
    };

    Some(SignatureHelp {
        name: doc.name.to_string(),
        kind: DocKind::Function,
        signature: doc.signature.to_string(),
        active_param,
        params,
        help: UiMessage::new(format!("formula-fn-{}", doc.name), doc.description),
    })
}
