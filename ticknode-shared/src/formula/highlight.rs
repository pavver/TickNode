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

/// Produces contiguous, ordered syntax highlights covering all of `source`.
pub fn highlight(source: &str, _inputs: &[String]) -> Vec<Highlight> {
    let mut builder = HighlightBuilder::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Whitespace
        if bytes[i].is_ascii_whitespace() {
            let start = i;
            while i < len && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            builder.push(start..i, TokenClass::Text);
            continue;
        }

        // Numbers (starting with a digit)
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
            if i + 1 < len && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
                i += 2;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
            }
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
                } else {
                    i = e_pos;
                }
            }
            builder.push(start..i, TokenClass::Number);
            continue;
        }

        // Numbers starting with decimal point followed by digit
        if bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
            let start = i;
            i += 2;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
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
                } else {
                    i = e_pos;
                }
            }
            builder.push(start..i, TokenClass::Number);
            continue;
        }

        // Identifiers (functions, constants, inputs, operator keywords)
        if is_ident_start(bytes[i]) {
            let start = i;
            while i < len && is_ident_continue(bytes[i]) {
                i += 1;
                if i + 1 < len && bytes[i] == b':' && bytes[i + 1] == b':' {
                    i += 2;
                }
            }
            let ident = &source[start..i];

            // Check if followed by '(' (ignoring whitespace)
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

            builder.push(start..i, class);
            continue;
        }

        // Multi-character operators
        if bytes[i..].starts_with(b"**")
            || bytes[i..].starts_with(b"==")
            || bytes[i..].starts_with(b"!=")
            || bytes[i..].starts_with(b"<=")
            || bytes[i..].starts_with(b">=")
            || bytes[i..].starts_with(b"&&")
            || bytes[i..].starts_with(b"||")
        {
            builder.push(i..i + 2, TokenClass::Operator);
            i += 2;
            continue;
        }

        // Single-character operators and delimiters
        if matches!(
            bytes[i],
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
        ) {
            builder.push(i..i + 1, TokenClass::Operator);
            i += 1;
            continue;
        }

        // Unrecognized or invalid character
        let start = i;
        if let Some(ch) = source[i..].chars().next() {
            i += ch.len_utf8();
        } else {
            i += 1;
        }
        builder.push(start..i, TokenClass::Error);
    }

    builder.finish()
}
