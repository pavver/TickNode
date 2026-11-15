//! Resilient syntax tokenizer for mathematical formulas.

use super::catalog::find_function;
use std::ops::Range;

#[cfg(test)]
mod tests;

/// Token classification for formula highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenClass {
    Text,
    Number,
    Operator,
    Function,
    Constant,
    Input,
    Error,
}

/// A classified range of bytes within a formula source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlight {
    pub range: Range<usize>,
    pub class: TokenClass,
}

struct HighlightBuilder {
    highlights: Vec<Highlight>,
}

impl HighlightBuilder {
    fn new() -> Self {
        Self {
            highlights: Vec::new(),
        }
    }

    fn push(&mut self, range: Range<usize>, class: TokenClass) {
        if range.is_empty() {
            return;
        }
        if let Some(last) = self.highlights.last_mut()
            && last.class == class
            && last.range.end == range.start
        {
            last.range.end = range.end;
            return;
        }
        self.highlights.push(Highlight { range, class });
    }

    fn finish(self) -> Vec<Highlight> {
        self.highlights
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_constant(name: &str) -> bool {
    matches!(name, "pi" | "e" | "tau" | "inf" | "nan" | "true" | "false")
}

fn is_operator_keyword(name: &str) -> bool {
    matches!(name, "and" | "or" | "not")
}

fn scan_exponent(bytes: &[u8], mut i: usize, len: usize) -> usize {
    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let e_pos = i;
        i += 1;
        if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        if i < len && bytes[i].is_ascii_digit() {
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
            i
        } else {
            e_pos
        }
    } else {
        i
    }
}

fn scan_number(bytes: &[u8], mut i: usize, len: usize) -> usize {
    if bytes[i].is_ascii_digit() {
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if i + 1 < len && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 2;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
        }
        scan_exponent(bytes, i, len)
    } else if bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
        i += 2;
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        scan_exponent(bytes, i, len)
    } else {
        i
    }
}

fn scan_identifier(source: &str, bytes: &[u8], mut i: usize, len: usize) -> (usize, TokenClass) {
    let start = i;
    while i < len
        && (is_ident_continue(bytes[i])
            || (bytes[i] == b'.' && i + 1 < len && is_ident_start(bytes[i + 1])))
    {
        i += 1;
        if i + 1 < len && bytes[i] == b':' && bytes[i + 1] == b':' {
            i += 2;
        }
    }
    let ident = &source[start..i];

    let mut next_k = i;
    while next_k < len && bytes[next_k].is_ascii_whitespace() {
        next_k += 1;
    }
    let is_call = next_k < len && bytes[next_k] == b'(';

    let class = if is_operator_keyword(ident) {
        TokenClass::Operator
    } else if is_constant(ident) {
        TokenClass::Constant
    } else if find_function(ident).is_some() || is_call {
        TokenClass::Function
    } else {
        TokenClass::Input
    };
    (i, class)
}

fn scan_multi_operator(bytes: &[u8], i: usize) -> Option<usize> {
    if bytes[i..].starts_with(b"**")
        || bytes[i..].starts_with(b"==")
        || bytes[i..].starts_with(b"!=")
        || bytes[i..].starts_with(b"<=")
        || bytes[i..].starts_with(b">=")
        || bytes[i..].starts_with(b"&&")
        || bytes[i..].starts_with(b"||")
    {
        Some(i + 2)
    } else {
        None
    }
}

fn is_single_operator(b: u8) -> bool {
    matches!(
        b,
        b'+' | b'-'
            | b'*'
            | b'/'
            | b'%'
            | b'^'
            | b'<'
            | b'>'
            | b'!'
            | b'?'
            | b':'
            | b','
            | b'('
            | b')'
    )
}

/// Produces contiguous, ordered syntax highlights covering all of `source`.
pub fn highlight(source: &str, _inputs: &[String]) -> Vec<Highlight> {
    let mut builder = HighlightBuilder::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i].is_ascii_whitespace() {
            let start = i;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            builder.push(start..i, TokenClass::Text);
        } else if bytes[i].is_ascii_digit()
            || (bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            let start = i;
            i = scan_number(bytes, i, len);
            builder.push(start..i, TokenClass::Number);
        } else if is_ident_start(bytes[i]) {
            let start = i;
            let (end, class) = scan_identifier(source, bytes, i, len);
            i = end;
            builder.push(start..i, class);
        } else if let Some(end) = scan_multi_operator(bytes, i) {
            builder.push(i..end, TokenClass::Operator);
            i = end;
        } else if is_single_operator(bytes[i]) {
            builder.push(i..i + 1, TokenClass::Operator);
            i += 1;
        } else {
            let start = i;
            if let Some(ch) = source[i..].chars().next() {
                i += ch.len_utf8();
            } else {
                i += 1;
            }
            builder.push(start..i, TokenClass::Error);
        }
    }

    builder.finish()
}
