use super::lexer::{Token, tokenize};
use super::{
    CompareOp, Condition, ConditionError, Expected, Join, MatchCase, MatchInput, Operand,
    is_input_name, parse_input_type,
};

impl MatchCase {
    pub fn parse(text: &str) -> Result<MatchCase, ConditionError> {
        let (pin, rest) = text.split_once(':').ok_or(ConditionError::MissingPin)?;
        let pin: u32 = pin
            .trim()
            .parse()
            .map_err(|_| ConditionError::NotPinNumber(pin.trim().to_string()))?;
        Ok(MatchCase {
            pin,
            conditions: parse_conditions(rest)?,
        })
    }
}

impl MatchInput {
    pub fn parse(text: &str) -> Result<MatchInput, ConditionError> {
        let mut parts = text.splitn(3, ':');
        let (Some(pin), Some(name), Some(ty)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(ConditionError::InputFormat(text.to_string()));
        };
        let pin: u32 = pin
            .trim()
            .parse()
            .map_err(|_| ConditionError::NotPinNumber(pin.trim().to_string()))?;
        let name = name.trim().to_string();
        if !is_input_name(&name) {
            return Err(ConditionError::InvalidInputName(name));
        }
        let value_type = parse_input_type(ty)
            .ok_or_else(|| ConditionError::UnknownType(ty.trim().to_string()))?;
        Ok(MatchInput {
            pin,
            name,
            value_type,
        })
    }
}

fn parse_conditions(text: &str) -> Result<Vec<Condition>, ConditionError> {
    let mut tokens = tokenize(text)?.into_iter().peekable();
    let mut conditions = Vec::new();
    while tokens.peek().is_some() {
        let join = parse_join(conditions.is_empty(), &mut tokens)?;
        let left = parse_operand(tokens.next())?;
        let op = parse_operator(tokens.next())?;
        let right = if op.is_unary() {
            None
        } else {
            Some(parse_operand(tokens.next())?)
        };
        conditions.push(Condition {
            join,
            left,
            op,
            right,
        });
    }
    Ok(conditions)
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

fn parse_join(
    first: bool,
    tokens: &mut impl Iterator<Item = Token>,
) -> Result<Join, ConditionError> {
    if first {
        return Ok(Join::And);
    }
    match tokens.next() {
        Some(Token::Join(join)) => Ok(join),
        other => Err(ConditionError::Expected(
            Expected::Join,
            found(other.as_ref()),
        )),
    }
}

fn parse_operand(token: Option<Token>) -> Result<Operand, ConditionError> {
    token
        .clone()
        .and_then(Token::into_operand)
        .ok_or_else(|| ConditionError::Expected(Expected::Operand, found(token.as_ref())))
}

fn parse_operator(token: Option<Token>) -> Result<CompareOp, ConditionError> {
    match token {
        Some(Token::Op(op)) => Ok(op),
        Some(Token::Ident(word)) => {
            CompareOp::parse(&word).ok_or(ConditionError::UnknownOperator(word))
        }
        other => Err(ConditionError::Expected(
            Expected::Operator,
            found(other.as_ref()),
        )),
    }
}
