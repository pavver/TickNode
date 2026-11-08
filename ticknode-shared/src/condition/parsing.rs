use crate::{DataValue, NumberValue};
use super::{CompareOp, Condition, ConditionError, Expected, Join, Operand, MatchCase, MatchInput, is_input_name, parse_input_type};

impl MatchCase {
    pub fn parse(text: &str) -> Result<MatchCase, ConditionError> {
        let (pin, rest) = text.split_once(':').ok_or(ConditionError::MissingPin)?;
        let pin: u32 = pin.trim().parse().map_err(|_| ConditionError::NotPinNumber(pin.trim().to_string()))?;
        Ok(MatchCase { pin, conditions: parse_conditions(rest)? })
    }
}

impl MatchInput {
    pub fn parse(text: &str) -> Result<MatchInput, ConditionError> {
        let mut parts = text.splitn(3, ':');
        let (Some(pin), Some(name), Some(ty)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(ConditionError::InputFormat(text.to_string()));
        };
        let pin: u32 = pin.trim().parse().map_err(|_| ConditionError::NotPinNumber(pin.trim().to_string()))?;
        let name = name.trim().to_string();
        if !is_input_name(&name) {
            return Err(ConditionError::InvalidInputName(name));
        }
        let value_type = parse_input_type(ty).ok_or_else(|| ConditionError::UnknownType(ty.trim().to_string()))?;
        Ok(MatchInput { pin, name, value_type })
    }
}

fn parse_conditions(text: &str) -> Result<Vec<Condition>, ConditionError> {
    let mut tokens = tokenize(text)?.into_iter().peekable();
    let mut conditions = Vec::new();
    while tokens.peek().is_some() {
        let join = if conditions.is_empty() {
            Join::And
        } else {
            match tokens.next() {
                Some(Token::Join(j)) => j,
                other => return Err(ConditionError::Expected(Expected::Join, found(other.as_ref()))),
            }
        };
        let operand = |token: Option<Token>| {
            token.clone().and_then(Token::into_operand).ok_or_else(|| ConditionError::Expected(Expected::Operand, found(token.as_ref())))
        };
        let left = operand(tokens.next())?;
        let op = match tokens.next() {
            Some(Token::Op(op)) => op,
            Some(Token::Ident(word)) => CompareOp::parse(&word).ok_or_else(|| ConditionError::UnknownOperator(word))?,
            other => return Err(ConditionError::Expected(Expected::Operator, found(other.as_ref()))),
        };
        let right = if op.is_unary() { None } else { Some(operand(tokens.next())?) };
        conditions.push(Condition { join, left, op, right });
    }
    Ok(conditions)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Value(DataValue),
    Op(CompareOp),
    Join(Join),
}

impl Token {
    fn into_operand(self) -> Option<Operand> {
        match self {
            Token::Ident(name) if is_input_name(&name) => Some(Operand::Input(name)),
            Token::Value(v) => Some(Operand::Value(v)),
            _ => None,
        }
    }
}

/// What was found instead of the expected token: `None` is the end of the text.
fn found(token: Option<&Token>) -> Option<String> {
    token.map(|t| match t {
        Token::Ident(s) => s.clone(),
        Token::Value(v) => Operand::Value(v.clone()).to_text(),
        Token::Op(op) => op.symbol().to_string(),
        Token::Join(j) => j.symbol().to_string(),
    })
}

fn tokenize(text: &str) -> Result<Vec<Token>, ConditionError> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let mut s = String::new();
            i += 1;
            loop {
                match chars.get(i) {
                    None => return Err(ConditionError::UnclosedQuote),
                    Some('"') => break,
                    Some('\\') => {
                        s.extend(chars.get(i + 1));
                        i += 2;
                    }
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            i += 1;
            tokens.push(Token::Value(DataValue::String(s)));
        } else if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            i += 1;
            while chars.get(i).is_some_and(|d| d.is_ascii_digit() || *d == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let f: f64 = s.parse().map_err(|_| ConditionError::NotNumber(s))?;
            tokens.push(Token::Value(DataValue::Number(NumberValue::Float(f).normalize())));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while chars.get(i).is_some_and(|d| d.is_alphanumeric() || *d == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(match word.as_str() {
                "true" => Token::Value(DataValue::Bool(true)),
                "false" => Token::Value(DataValue::Bool(false)),
                "and" | "і" | "та" => Token::Join(Join::And),
                "or" | "або" => Token::Join(Join::Or),
                _ => Token::Ident(word),
            });
        } else if c == '&' || c == '|' {
            if chars.get(i + 1) != Some(&c) {
                return Err(ConditionError::DoubledOperator(c));
            }
            i += 2;
            tokens.push(Token::Join(if c == '&' { Join::And } else { Join::Or }));
        } else {
            let two: String = chars[i..(i + 2).min(chars.len())].iter().collect();
            let (op, len) = match CompareOp::parse(&two) {
                Some(op) if two.chars().count() == 2 => (op, 2),
                _ => (CompareOp::parse(&c.to_string()).ok_or(ConditionError::UnexpectedChar(c))?, 1),
            };
            i += len;
            tokens.push(Token::Op(op));
        }
    }
    Ok(tokens)
}
