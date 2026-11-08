use crate::{DataValue, NumberValue};
use super::{CompareOp, ConditionError, Join, Operand, is_input_name};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    Ident(String),
    Value(DataValue),
    Op(CompareOp),
    Join(Join),
}

impl Token {
    pub(super) fn into_operand(self) -> Option<Operand> {
        match self {
            Token::Ident(name) if is_input_name(&name) => Some(Operand::Input(name)),
            Token::Value(v) => Some(Operand::Value(v)),
            _ => None,
        }
    }
}

pub(super) fn tokenize(text: &str) -> Result<Vec<Token>, ConditionError> {
    let mut lexer = Lexer { chars: text.chars().collect(), position: 0 };
    let mut tokens = Vec::new();
    while let Some(&ch) = lexer.chars.get(lexer.position) {
        if ch.is_whitespace() {
            lexer.position += 1;
        } else {
            tokens.push(lexer.token(ch)?);
        }
    }
    Ok(tokens)
}

struct Lexer {
    chars: Vec<char>,
    position: usize,
}

impl Lexer {
    fn token(&mut self, ch: char) -> Result<Token, ConditionError> {
        if ch == '"' { return self.string(); }
        if ch.is_ascii_digit() || (ch == '-' && self.chars.get(self.position + 1).is_some_and(|d| d.is_ascii_digit())) {
            return self.number();
        }
        if ch.is_alphabetic() || ch == '_' { return Ok(self.word()); }
        if ch == '&' || ch == '|' { return self.join(ch); }
        self.operator(ch)
    }

    fn string(&mut self) -> Result<Token, ConditionError> {
        let mut value = String::new();
        self.position += 1;
        loop {
            match self.chars.get(self.position) {
                None => return Err(ConditionError::UnclosedQuote),
                Some('"') => break,
                Some('\\') => {
                    value.extend(self.chars.get(self.position + 1));
                    self.position += 2;
                }
                Some(ch) => {
                    value.push(*ch);
                    self.position += 1;
                }
            }
        }
        self.position += 1;
        Ok(Token::Value(DataValue::String(value)))
    }

    fn number(&mut self) -> Result<Token, ConditionError> {
        let start = self.position;
        self.position += 1;
        while self.chars.get(self.position).is_some_and(|ch| ch.is_ascii_digit() || *ch == '.') {
            self.position += 1;
        }
        let text: String = self.chars[start..self.position].iter().collect();
        let number: f64 = text.parse().map_err(|_| ConditionError::NotNumber(text))?;
        Ok(Token::Value(DataValue::Number(NumberValue::Float(number).normalize())))
    }

    fn word(&mut self) -> Token {
        let start = self.position;
        while self.chars.get(self.position).is_some_and(|ch| ch.is_alphanumeric() || *ch == '_') {
            self.position += 1;
        }
        let word: String = self.chars[start..self.position].iter().collect();
        match word.as_str() {
            "true" => Token::Value(DataValue::Bool(true)),
            "false" => Token::Value(DataValue::Bool(false)),
            "and" | "і" | "та" => Token::Join(Join::And),
            "or" | "або" => Token::Join(Join::Or),
            _ => Token::Ident(word),
        }
    }

    fn join(&mut self, ch: char) -> Result<Token, ConditionError> {
        if self.chars.get(self.position + 1) != Some(&ch) {
            return Err(ConditionError::DoubledOperator(ch));
        }
        self.position += 2;
        Ok(Token::Join(if ch == '&' { Join::And } else { Join::Or }))
    }

    fn operator(&mut self, ch: char) -> Result<Token, ConditionError> {
        let end = (self.position + 2).min(self.chars.len());
        let two: String = self.chars[self.position..end].iter().collect();
        let (op, len) = match CompareOp::parse(&two) {
            Some(op) if two.chars().count() == 2 => (op, 2),
            _ => (CompareOp::parse(&ch.to_string()).ok_or(ConditionError::UnexpectedChar(ch))?, 1),
        };
        self.position += len;
        Ok(Token::Op(op))
    }
}
