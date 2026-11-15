use super::error::FormulaError;
use crate::NumberValue;
use std::ops::Range;

pub const MAX_SOURCE_LEN: usize = 4096;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(NumberValue),
    Bool(bool),
    Ident(String),
    VarRef(crate::VariableRef),
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    Percent,
    Caret,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    AmpAmp,
    PipePipe,
    Bang,
    Question,
    Colon,
    Comma,
    LParen,
    RParen,
    Eof,
}

pub struct Lexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Result<Self, FormulaError> {
        if source.len() > MAX_SOURCE_LEN {
            return Err(FormulaError::too_large(source.len(), MAX_SOURCE_LEN));
        }
        Ok(Self {
            source,
            bytes: source.as_bytes(),
            cursor: 0,
        })
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, FormulaError> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            if self.is_eof() {
                tokens.push(Token {
                    kind: TokenKind::Eof,
                    span: self.cursor..self.cursor,
                });
                break;
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while !self.is_eof() && self.current_byte().is_ascii_whitespace() {
            self.cursor += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.bytes.len()
    }

    fn current_byte(&self) -> u8 {
        self.bytes[self.cursor]
    }

    fn peek_byte(&self) -> Option<u8> {
        if self.cursor + 1 < self.bytes.len() {
            Some(self.bytes[self.cursor + 1])
        } else {
            None
        }
    }

    fn next_token(&mut self) -> Result<Token, FormulaError> {
        let start = self.cursor;
        let b = self.current_byte();

        // Multi-byte or single-byte symbols
        match b {
            b'+' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Plus,
                    span: start..self.cursor,
                })
            }
            b'-' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Minus,
                    span: start..self.cursor,
                })
            }
            b'*' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'*' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::StarStar,
                        span: start..self.cursor,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Star,
                        span: start..self.cursor,
                    })
                }
            }
            b'/' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Slash,
                    span: start..self.cursor,
                })
            }
            b'%' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Percent,
                    span: start..self.cursor,
                })
            }
            b'^' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Caret,
                    span: start..self.cursor,
                })
            }
            b'=' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'=' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::EqEq,
                        span: start..self.cursor,
                    })
                } else {
                    Err(FormulaError::unexpected_token(
                        start..self.cursor,
                        &self.source[start..self.cursor],
                    ))
                }
            }
            b'!' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'=' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::BangEq,
                        span: start..self.cursor,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Bang,
                        span: start..self.cursor,
                    })
                }
            }
            b'<' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'=' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::LtEq,
                        span: start..self.cursor,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Lt,
                        span: start..self.cursor,
                    })
                }
            }
            b'>' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'=' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::GtEq,
                        span: start..self.cursor,
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Gt,
                        span: start..self.cursor,
                    })
                }
            }
            b'&' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'&' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::AmpAmp,
                        span: start..self.cursor,
                    })
                } else {
                    Err(FormulaError::unexpected_token(
                        start..self.cursor,
                        &self.source[start..self.cursor],
                    ))
                }
            }
            b'|' => {
                self.cursor += 1;
                if !self.is_eof() && self.current_byte() == b'|' {
                    self.cursor += 1;
                    Ok(Token {
                        kind: TokenKind::PipePipe,
                        span: start..self.cursor,
                    })
                } else {
                    Err(FormulaError::unexpected_token(
                        start..self.cursor,
                        &self.source[start..self.cursor],
                    ))
                }
            }
            b'?' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Question,
                    span: start..self.cursor,
                })
            }
            b':' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Colon,
                    span: start..self.cursor,
                })
            }
            b',' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::Comma,
                    span: start..self.cursor,
                })
            }
            b'(' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::LParen,
                    span: start..self.cursor,
                })
            }
            b')' => {
                self.cursor += 1;
                Ok(Token {
                    kind: TokenKind::RParen,
                    span: start..self.cursor,
                })
            }
            b'0'..=b'9' => self.lex_number(start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_ident_or_keyword(start),
            _ => {
                // Determine character boundary for utf-8
                let mut end = self.cursor + 1;
                while end < self.bytes.len() && !self.source.is_char_boundary(end) {
                    end += 1;
                }
                self.cursor = end;
                Err(FormulaError::unexpected_token(
                    start..end,
                    &self.source[start..end],
                ))
            }
        }
    }

    fn lex_number(&mut self, start: usize) -> Result<Token, FormulaError> {
        let mut is_float = false;
        while !self.is_eof() {
            let b = self.current_byte();
            if b.is_ascii_digit() || b == b'_' {
                self.cursor += 1;
            } else if b == b'.' {
                // Only consider decimal point if followed by digit
                if self.peek_byte().is_some_and(|next| next.is_ascii_digit()) {
                    is_float = true;
                    self.cursor += 2; // skip '.' and first digit
                } else {
                    break;
                }
            } else if b == b'e' || b == b'E' {
                is_float = true;
                self.cursor += 1;
                if !self.is_eof() && (self.current_byte() == b'+' || self.current_byte() == b'-') {
                    self.cursor += 1;
                }
            } else {
                break;
            }
        }

        let slice = &self.source[start..self.cursor];
        let cleaned: String = slice.chars().filter(|&c| c != '_').collect();

        if is_float {
            let val = cleaned
                .parse::<f64>()
                .map_err(|_| FormulaError::unexpected_token(start..self.cursor, slice))?;
            Ok(Token {
                kind: TokenKind::Number(NumberValue::Float(val).normalize()),
                span: start..self.cursor,
            })
        } else if let Ok(val) = cleaned.parse::<i64>() {
            Ok(Token {
                kind: TokenKind::Number(NumberValue::Int(val)),
                span: start..self.cursor,
            })
        } else if let Ok(val) = cleaned.parse::<f64>() {
            Ok(Token {
                kind: TokenKind::Number(NumberValue::Float(val)),
                span: start..self.cursor,
            })
        } else {
            Err(FormulaError::unexpected_token(start..self.cursor, slice))
        }
    }

    fn lex_ident_or_keyword(&mut self, start: usize) -> Result<Token, FormulaError> {
        let mut saw_dot = false;
        while !self.is_eof() {
            let b = self.current_byte();
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.cursor += 1;
            } else if b == b':' && self.peek_byte() == Some(b':') {
                // Allow `math::name` for evalexpr compatibility
                self.cursor += 2;
            } else if b == b'.' {
                // If followed by identifier start, part of dotted name
                if self
                    .peek_byte()
                    .is_some_and(|next| next.is_ascii_alphabetic() || next == b'_')
                {
                    saw_dot = true;
                    self.cursor += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let slice = &self.source[start..self.cursor];
        if saw_dot {
            if let Some(vref) = crate::VariableRef::parse(slice) {
                return Ok(Token {
                    kind: TokenKind::VarRef(vref),
                    span: start..self.cursor,
                });
            }
            return Err(FormulaError::reserved_name(start..self.cursor, slice));
        }

        let kind = match slice {
            "true" => TokenKind::Bool(true),
            "false" => TokenKind::Bool(false),
            "and" => TokenKind::AmpAmp,
            "or" => TokenKind::PipePipe,
            "not" => TokenKind::Bang,
            _ => TokenKind::Ident(slice.to_string()),
        };

        Ok(Token {
            kind,
            span: start..self.cursor,
        })
    }
}
