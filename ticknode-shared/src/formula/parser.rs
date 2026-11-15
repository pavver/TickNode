use super::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use super::catalog::find_function;
use super::error::FormulaError;
use super::lexer::{Token, TokenKind};
use crate::NumberValue;
use std::ops::Range;

pub const MAX_NESTING_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest = 0,
    Ternary = 1,
    Or = 2,
    And = 3,
    Equality = 4,
    Compare = 5,
    Sum = 6,
    Product = 7,
    Unary = 8,
    Power = 9,
}

pub struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, cursor: 0 }
    }

    pub fn parse(&mut self) -> Result<Expr, FormulaError> {
        if self.peek().kind == TokenKind::Eof {
            return Err(FormulaError::unexpected_token(0..0, "end of formula"));
        }
        let expr = self.parse_expr(Precedence::Lowest, 0)?;
        if self.peek().kind != TokenKind::Eof {
            let tok = self.peek();
            return Err(FormulaError::unexpected_token(
                tok.span.clone(),
                &format!("{:?}", tok.kind),
            ));
        }
        Ok(expr)
    }

    fn peek(&self) -> &Token {
        if self.cursor < self.tokens.len() {
            &self.tokens[self.cursor]
        } else {
            &self.tokens[self.tokens.len() - 1]
        }
    }

    fn next(&mut self) -> Token {
        let tok = self.peek().clone();
        if self.cursor < self.tokens.len() {
            self.cursor += 1;
        }
        tok
    }

    fn parse_expr(&mut self, min_prec: Precedence, depth: usize) -> Result<Expr, FormulaError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(FormulaError::depth_exceeded(
                self.peek().span.clone(),
                MAX_NESTING_DEPTH,
            ));
        }

        let mut left = self.parse_prefix(depth + 1)?;

        while !self.is_at_end() {
            let next_tok = self.peek();

            if next_tok.kind == TokenKind::Question {
                if Precedence::Ternary < min_prec {
                    break;
                }
                left = self.parse_ternary(left, depth + 1)?;
                continue;
            }

            let Some((op, prec)) = binary_op(&next_tok.kind) else {
                break;
            };

            if prec < min_prec {
                break;
            }

            self.next(); // consume operator

            // Power is right-associative; other binary operators are left-associative
            let next_min_prec = if op == BinaryOp::Pow {
                prec
            } else {
                next_precedence(prec)
            };

            let right = self.parse_expr(next_min_prec, depth + 1)?;
            let span = left.span.start..right.span.end;
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }

        Ok(left)
    }

    fn parse_prefix(&mut self, depth: usize) -> Result<Expr, FormulaError> {
        let token = self.next();
        match token.kind {
            TokenKind::Number(num) => Ok(Expr {
                kind: ExprKind::Number(num),
                span: token.span,
            }),
            TokenKind::Bool(b) => Ok(Expr {
                kind: ExprKind::Bool(b),
                span: token.span,
            }),
            TokenKind::Ident(ref name) => self.parse_ident(name, token.span, depth),
            TokenKind::Minus => {
                let right = self.parse_expr(Precedence::Unary, depth + 1)?;
                let span = token.span.start..right.span.end;
                Ok(Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Neg,
                        expr: Box::new(right),
                    },
                    span,
                })
            }
            TokenKind::Plus => {
                let right = self.parse_expr(Precedence::Unary, depth + 1)?;
                let span = token.span.start..right.span.end;
                Ok(Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Pos,
                        expr: Box::new(right),
                    },
                    span,
                })
            }
            TokenKind::Bang => {
                let right = self.parse_expr(Precedence::Unary, depth + 1)?;
                let span = token.span.start..right.span.end;
                Ok(Expr {
                    kind: ExprKind::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(right),
                    },
                    span,
                })
            }
            TokenKind::LParen => {
                let expr = self.parse_expr(Precedence::Lowest, depth + 1)?;
                if self.peek().kind != TokenKind::RParen {
                    return Err(FormulaError::unclosed_paren(
                        token.span.start..expr.span.end,
                    ));
                }
                let close = self.next();
                Ok(Expr {
                    kind: expr.kind,
                    span: token.span.start..close.span.end,
                })
            }
            _ => Err(FormulaError::unexpected_token(
                token.span,
                &format!("{:?}", token.kind),
            )),
        }
    }

    fn parse_ident(
        &mut self,
        name: &str,
        span: Range<usize>,
        depth: usize,
    ) -> Result<Expr, FormulaError> {
        if self.peek().kind == TokenKind::LParen {
            self.parse_call(name, span, depth)
        } else {
            // Check mathematical constants
            match name {
                "pi" => Ok(Expr {
                    kind: ExprKind::Number(NumberValue::Float(std::f64::consts::PI)),
                    span,
                }),
                "e" => Ok(Expr {
                    kind: ExprKind::Number(NumberValue::Float(std::f64::consts::E)),
                    span,
                }),
                "tau" => Ok(Expr {
                    kind: ExprKind::Number(NumberValue::Float(std::f64::consts::TAU)),
                    span,
                }),
                "inf" => Ok(Expr {
                    kind: ExprKind::Number(NumberValue::Float(f64::INFINITY)),
                    span,
                }),
                "nan" => Ok(Expr {
                    kind: ExprKind::Number(NumberValue::Float(f64::NAN)),
                    span,
                }),
                _ => {
                    // If name is a function from catalog used without parentheses, report syntax error
                    if find_function(name).is_some() {
                        return Err(FormulaError::unexpected_token(span, name));
                    }
                    Ok(Expr {
                        kind: ExprKind::Variable(name.to_string()),
                        span,
                    })
                }
            }
        }
    }

    fn parse_call(
        &mut self,
        name: &str,
        span: Range<usize>,
        depth: usize,
    ) -> Result<Expr, FormulaError> {
        let lparen = self.next(); // consume '('
        let doc = find_function(name).ok_or_else(|| {
            if let Some(sugg) = crate::template::lint::find_suggestion(
                name,
                super::catalog::CATALOG.iter().map(|d| d.name),
            ) {
                FormulaError::unknown_function_suggest(span.start..lparen.span.end, name, &sugg)
            } else {
                FormulaError::unknown_function(span.start..lparen.span.end, name)
            }
        })?;

        let mut args = Vec::new();
        if self.peek().kind != TokenKind::RParen {
            loop {
                let arg = self.parse_expr(Precedence::Lowest, depth + 1)?;
                args.push(arg);
                if self.peek().kind == TokenKind::Comma {
                    self.next();
                } else {
                    break;
                }
            }
        }

        if self.peek().kind != TokenKind::RParen {
            return Err(FormulaError::unclosed_paren(
                span.start..self.peek().span.end,
            ));
        }
        let rparen = self.next();
        let total_span = span.start..rparen.span.end;

        // Check arity
        let got = args.len();
        if got < doc.min_args || doc.max_args.is_some_and(|max| got > max) {
            let expected = match doc.max_args {
                Some(max) if max == doc.min_args => format!("{}", doc.min_args),
                Some(max) => format!("{}-{}", doc.min_args, max),
                None => format!("at least {}", doc.min_args),
            };
            return Err(FormulaError::wrong_arity(
                total_span, doc.name, &expected, got,
            ));
        }

        Ok(Expr {
            kind: ExprKind::Call {
                name: doc.name.to_string(),
                args,
            },
            span: total_span,
        })
    }

    fn parse_ternary(&mut self, cond: Expr, depth: usize) -> Result<Expr, FormulaError> {
        self.next(); // consume '?'
        let then_expr = self.parse_expr(Precedence::Lowest, depth + 1)?;
        if self.peek().kind != TokenKind::Colon {
            return Err(FormulaError::unexpected_token(
                self.peek().span.clone(),
                ":",
            ));
        }
        self.next(); // consume ':'
        let else_expr = self.parse_expr(Precedence::Ternary, depth + 1)?;
        let span = cond.span.start..else_expr.span.end;
        Ok(Expr {
            kind: ExprKind::Ternary {
                cond: Box::new(cond),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            },
            span,
        })
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }
}

fn binary_op(tok: &TokenKind) -> Option<(BinaryOp, Precedence)> {
    match tok {
        TokenKind::PipePipe => Some((BinaryOp::Or, Precedence::Or)),
        TokenKind::AmpAmp => Some((BinaryOp::And, Precedence::And)),
        TokenKind::EqEq => Some((BinaryOp::Eq, Precedence::Equality)),
        TokenKind::BangEq => Some((BinaryOp::Ne, Precedence::Equality)),
        TokenKind::Lt => Some((BinaryOp::Lt, Precedence::Compare)),
        TokenKind::LtEq => Some((BinaryOp::Le, Precedence::Compare)),
        TokenKind::Gt => Some((BinaryOp::Gt, Precedence::Compare)),
        TokenKind::GtEq => Some((BinaryOp::Ge, Precedence::Compare)),
        TokenKind::Plus => Some((BinaryOp::Add, Precedence::Sum)),
        TokenKind::Minus => Some((BinaryOp::Sub, Precedence::Sum)),
        TokenKind::Star => Some((BinaryOp::Mul, Precedence::Product)),
        TokenKind::Slash => Some((BinaryOp::Div, Precedence::Product)),
        TokenKind::Percent => Some((BinaryOp::Rem, Precedence::Product)),
        TokenKind::Caret | TokenKind::StarStar => Some((BinaryOp::Pow, Precedence::Power)),
        _ => None,
    }
}

fn next_precedence(prec: Precedence) -> Precedence {
    match prec {
        Precedence::Lowest => Precedence::Ternary,
        Precedence::Ternary => Precedence::Or,
        Precedence::Or => Precedence::And,
        Precedence::And => Precedence::Equality,
        Precedence::Equality => Precedence::Compare,
        Precedence::Compare => Precedence::Sum,
        Precedence::Sum => Precedence::Product,
        Precedence::Product => Precedence::Unary,
        Precedence::Unary => Precedence::Power,
        Precedence::Power => Precedence::Power,
    }
}
