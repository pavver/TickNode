//! Hand-written resilient syntax tokenizer for MiniJinja templates.

use std::ops::Range;

#[cfg(test)]
mod tests;

/// Syntax token classification for template highlighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenClass {
    Text,
    Delimiter,
    Keyword,
    Input,
    Variable,
    Field,
    Filter,
    Test,
    Function,
    String,
    Number,
    Operator,
    Comment,
}

/// A classified range of bytes within a template source string.
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

#[derive(Default)]
struct TagContext {
    prev_was_dot: bool,
    prev_was_pipe: bool,
    after_is: bool,
    after_is_not: bool,
    tag_first_word: bool,
}

impl TagContext {
    fn reset_after_value(&mut self) {
        self.prev_was_dot = false;
        self.prev_was_pipe = false;
        self.after_is = false;
        self.after_is_not = false;
        self.tag_first_word = false;
    }

    fn on_dot(&mut self) {
        self.prev_was_dot = true;
        self.prev_was_pipe = false;
        self.after_is = false;
        self.after_is_not = false;
        self.tag_first_word = false;
    }

    fn on_pipe(&mut self) {
        self.prev_was_pipe = true;
        self.prev_was_dot = false;
        self.after_is = false;
        self.after_is_not = false;
        self.tag_first_word = false;
    }

    fn classify_ident(
        &mut self,
        ident: &str,
        source: &str,
        ident_end: usize,
        inputs: &[String],
    ) -> TokenClass {
        let class = if self.prev_was_dot {
            TokenClass::Field
        } else if self.prev_was_pipe {
            TokenClass::Filter
        } else if self.after_is {
            if ident == "not" {
                self.after_is = false;
                self.after_is_not = true;
                return TokenClass::Keyword;
            } else {
                TokenClass::Test
            }
        } else if self.after_is_not {
            TokenClass::Test
        } else if self.tag_first_word {
            TokenClass::Keyword
        } else if is_keyword(ident) {
            if ident == "is" {
                self.after_is = true;
                self.prev_was_dot = false;
                self.prev_was_pipe = false;
                self.tag_first_word = false;
                return TokenClass::Keyword;
            }
            TokenClass::Keyword
        } else if source[ident_end..].starts_with('(') {
            TokenClass::Function
        } else if inputs.iter().any(|inp| inp == ident) {
            TokenClass::Input
        } else {
            TokenClass::Variable
        };

        self.reset_after_value();
        class
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "and"
            | "or"
            | "not"
            | "in"
            | "is"
            | "if"
            | "else"
            | "elif"
            | "true"
            | "false"
            | "none"
            | "True"
            | "False"
            | "None"
            | "recursive"
            | "as"
            | "with"
    )
}

fn scan_number(source: &str, i: &mut usize) {
    let bytes = source.as_bytes();
    let len = bytes.len();
    if bytes[*i..].starts_with(b"0x") || bytes[*i..].starts_with(b"0X") {
        *i += 2;
        while *i < len && (bytes[*i].is_ascii_hexdigit() || bytes[*i] == b'_') {
            *i += 1;
        }
    } else if bytes[*i..].starts_with(b"0b") || bytes[*i..].starts_with(b"0B") {
        *i += 2;
        while *i < len && (bytes[*i] == b'0' || bytes[*i] == b'1' || bytes[*i] == b'_') {
            *i += 1;
        }
    } else if bytes[*i..].starts_with(b"0o") || bytes[*i..].starts_with(b"0O") {
        *i += 2;
        while *i < len && ((bytes[*i] >= b'0' && bytes[*i] <= b'7') || bytes[*i] == b'_') {
            *i += 1;
        }
    } else {
        while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
            *i += 1;
        }
        if *i + 1 < len && bytes[*i] == b'.' && bytes[*i + 1].is_ascii_digit() {
            *i += 1;
            while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
                *i += 1;
            }
        }
        if *i < len && (bytes[*i] == b'e' || bytes[*i] == b'E') {
            let mut peek = *i + 1;
            if peek < len && (bytes[peek] == b'+' || bytes[peek] == b'-') {
                peek += 1;
            }
            if peek < len && bytes[peek].is_ascii_digit() {
                *i = peek;
                while *i < len && (bytes[*i].is_ascii_digit() || bytes[*i] == b'_') {
                    *i += 1;
                }
            }
        }
    }
}

/// Tokenizes `source` into contiguous, non-overlapping, ordered byte ranges.
pub fn highlight(source: &str, inputs: &[String]) -> Vec<Highlight> {
    let mut builder = HighlightBuilder::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Plain text outside tags
        let plain_start = i;
        while i < len {
            if bytes[i..].starts_with(b"{#")
                || bytes[i..].starts_with(b"{{")
                || bytes[i..].starts_with(b"{%")
            {
                break;
            }
            if let Some(ch) = source[i..].chars().next() {
                i += ch.len_utf8();
            } else {
                i += 1;
            }
        }

        if plain_start < i {
            builder.push(plain_start..i, TokenClass::Text);
        }

        if i >= len {
            break;
        }

        if bytes[i..].starts_with(b"{#") {
            // Comment
            let comment_start = i;
            i += 2;
            if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
                i += 1;
            }
            while i < len {
                if bytes[i..].starts_with(b"-#}") || bytes[i..].starts_with(b"+#}") {
                    i += 3;
                    break;
                } else if bytes[i..].starts_with(b"#}") {
                    i += 2;
                    break;
                } else {
                    i += 1;
                }
            }
            builder.push(comment_start..i, TokenClass::Comment);
            continue;
        }

        let is_tag = bytes[i..].starts_with(b"{%");
        let delim_start = i;
        i += 2;
        if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
            i += 1;
        }
        builder.push(delim_start..i, TokenClass::Delimiter);

        let mut ctx = TagContext {
            tag_first_word: is_tag,
            ..Default::default()
        };

        // Scan inside interpolation / tag
        while i < len {
            if is_tag {
                if bytes[i..].starts_with(b"-%}") || bytes[i..].starts_with(b"+%}") {
                    builder.push(i..i + 3, TokenClass::Delimiter);
                    i += 3;
                    break;
                }
                if bytes[i..].starts_with(b"%}") {
                    builder.push(i..i + 2, TokenClass::Delimiter);
                    i += 2;
                    break;
                }
            } else {
                if bytes[i..].starts_with(b"-}}") || bytes[i..].starts_with(b"+}}") {
                    builder.push(i..i + 3, TokenClass::Delimiter);
                    i += 3;
                    break;
                }
                if bytes[i..].starts_with(b"}}") {
                    builder.push(i..i + 2, TokenClass::Delimiter);
                    i += 2;
                    break;
                }
            }

            if bytes[i].is_ascii_whitespace() {
                let ws_start = i;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                builder.push(ws_start..i, TokenClass::Text);
                continue;
            }

            if bytes[i] == b'\'' || bytes[i] == b'"' {
                let quote = bytes[i];
                let str_start = i;
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 1;
                        if i < len {
                            i += 1;
                        }
                    } else if bytes[i] == quote {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
                builder.push(str_start..i, TokenClass::String);
                ctx.reset_after_value();
                continue;
            }

            if bytes[i].is_ascii_digit() {
                let num_start = i;
                scan_number(source, &mut i);
                builder.push(num_start..i, TokenClass::Number);
                ctx.reset_after_value();
                continue;
            }

            // 2-character operators
            if bytes[i..].starts_with(b"//")
                || bytes[i..].starts_with(b"**")
                || bytes[i..].starts_with(b"==")
                || bytes[i..].starts_with(b"!=")
                || bytes[i..].starts_with(b"<=")
                || bytes[i..].starts_with(b">=")
            {
                builder.push(i..i + 2, TokenClass::Operator);
                i += 2;
                ctx.reset_after_value();
                continue;
            }

            if bytes[i] == b'.' {
                builder.push(i..i + 1, TokenClass::Operator);
                i += 1;
                ctx.on_dot();
                continue;
            }

            if bytes[i] == b'|' {
                builder.push(i..i + 1, TokenClass::Operator);
                i += 1;
                ctx.on_pipe();
                continue;
            }

            if matches!(
                bytes[i],
                b'+' | b'-'
                    | b'*'
                    | b'/'
                    | b'%'
                    | b'~'
                    | b'<'
                    | b'>'
                    | b'='
                    | b'('
                    | b')'
                    | b'['
                    | b']'
                    | b'{'
                    | b'}'
                    | b','
                    | b':'
            ) {
                builder.push(i..i + 1, TokenClass::Operator);
                i += 1;
                ctx.reset_after_value();
                continue;
            }

            if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                let ident_start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &source[ident_start..i];
                let class = ctx.classify_ident(ident, source, i, inputs);
                builder.push(ident_start..i, class);
                continue;
            }

            let start = i;
            if let Some(ch) = source[i..].chars().next() {
                i += ch.len_utf8();
            } else {
                i += 1;
            }
            builder.push(start..i, TokenClass::Text);
            ctx.reset_after_value();
        }
    }

    builder.finish()
}
