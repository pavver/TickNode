use super::ast::{BinaryOp, Expr, ExprKind, UnaryOp};
use super::error::FormulaError;
use crate::{DataValue, NumberValue};
use std::ops::Range;

mod functions;

#[derive(Debug, Clone, PartialEq)]
pub enum FormulaValue {
    Number(NumberValue),
    Bool(bool),
}

impl FormulaValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Number(NumberValue::Int(i)) => *i != 0,
            Self::Number(NumberValue::Float(f)) => *f != 0.0 && !f.is_nan(),
        }
    }

    pub fn to_number_value(&self) -> NumberValue {
        match self {
            Self::Number(n) => *n,
            Self::Bool(true) => NumberValue::Int(1),
            Self::Bool(false) => NumberValue::Int(0),
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.to_number_value().to_f64()
    }
}

impl From<NumberValue> for FormulaValue {
    fn from(n: NumberValue) -> Self {
        Self::Number(n)
    }
}

impl From<bool> for FormulaValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<FormulaValue> for DataValue {
    fn from(v: FormulaValue) -> Self {
        match v {
            FormulaValue::Number(n) => DataValue::Number(n),
            FormulaValue::Bool(b) => DataValue::Bool(b),
        }
    }
}

pub fn eval_expr<F>(expr: &Expr, lookup: &F) -> Result<FormulaValue, FormulaError>
where
    F: Fn(&str) -> Option<FormulaValue>,
{
    match &expr.kind {
        ExprKind::Number(num) => Ok(FormulaValue::Number(*num)),
        ExprKind::Bool(b) => Ok(FormulaValue::Bool(*b)),
        ExprKind::Variable(name) => {
            let val = lookup(name).unwrap_or(FormulaValue::Number(NumberValue::Int(0)));
            Ok(val)
        }
        ExprKind::Unary { op, expr: inner } => {
            let val = eval_expr(inner, lookup)?;
            match op {
                UnaryOp::Not => Ok(FormulaValue::Bool(!val.is_truthy())),
                UnaryOp::Pos => Ok(val),
                UnaryOp::Neg => {
                    let res = match val.to_number_value() {
                        NumberValue::Int(i) => i
                            .checked_neg()
                            .map(NumberValue::Int)
                            .unwrap_or_else(|| NumberValue::Float(-(i as f64))),
                        NumberValue::Float(f) => NumberValue::Float(-f).normalize(),
                    };
                    Ok(FormulaValue::Number(res))
                }
            }
        }
        ExprKind::Binary { op, left, right } => {
            // Short-circuiting logical operators
            if *op == BinaryOp::And {
                let l = eval_expr(left, lookup)?;
                if !l.is_truthy() {
                    return Ok(FormulaValue::Bool(false));
                }
                let r = eval_expr(right, lookup)?;
                return Ok(FormulaValue::Bool(r.is_truthy()));
            }
            if *op == BinaryOp::Or {
                let l = eval_expr(left, lookup)?;
                if l.is_truthy() {
                    return Ok(FormulaValue::Bool(true));
                }
                let r = eval_expr(right, lookup)?;
                return Ok(FormulaValue::Bool(r.is_truthy()));
            }

            let l = eval_expr(left, lookup)?;
            let r = eval_expr(right, lookup)?;
            eval_binary_op(*op, l, r, expr.span.clone())
        }
        ExprKind::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            let c = eval_expr(cond, lookup)?;
            if c.is_truthy() {
                eval_expr(then_expr, lookup)
            } else {
                eval_expr(else_expr, lookup)
            }
        }
        ExprKind::Call { name, args } => {
            // Short-circuiting if(c, a, b)
            if name == "if" {
                let cond = eval_expr(&args[0], lookup)?;
                return if cond.is_truthy() {
                    eval_expr(&args[1], lookup)
                } else {
                    eval_expr(&args[2], lookup)
                };
            }

            let vals: Vec<FormulaValue> = args
                .iter()
                .map(|a| eval_expr(a, lookup))
                .collect::<Result<_, _>>()?;

            let nums: Vec<NumberValue> = vals.iter().map(FormulaValue::to_number_value).collect();
            functions::eval_call(name, &nums, expr.span.clone())
        }
    }
}

fn eval_binary_op(
    op: BinaryOp,
    l: FormulaValue,
    r: FormulaValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    match op {
        BinaryOp::Eq => Ok(FormulaValue::Bool(values_equal(&l, &r))),
        BinaryOp::Ne => Ok(FormulaValue::Bool(!values_equal(&l, &r))),
        BinaryOp::Lt => Ok(FormulaValue::Bool(l.to_f64() < r.to_f64())),
        BinaryOp::Le => Ok(FormulaValue::Bool(l.to_f64() <= r.to_f64())),
        BinaryOp::Gt => Ok(FormulaValue::Bool(l.to_f64() > r.to_f64())),
        BinaryOp::Ge => Ok(FormulaValue::Bool(l.to_f64() >= r.to_f64())),
        BinaryOp::Add => eval_add(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::Sub => eval_sub(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::Mul => eval_mul(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::Div => eval_div(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::Rem => eval_rem(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::Pow => eval_pow(l.to_number_value(), r.to_number_value(), span),
        BinaryOp::And | BinaryOp::Or => unreachable!(),
    }
}

fn values_equal(l: &FormulaValue, r: &FormulaValue) -> bool {
    match (l, r) {
        (FormulaValue::Bool(a), FormulaValue::Bool(b)) => a == b,
        (FormulaValue::Number(a), FormulaValue::Number(b)) => match (a, b) {
            (NumberValue::Int(x), NumberValue::Int(y)) => x == y,
            _ => (a.to_f64() - b.to_f64()).abs() < 1e-12,
        },
        _ => l.is_truthy() == r.is_truthy(),
    }
}

fn eval_add(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) => {
            if let Some(res) = x.checked_add(y) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = x as f64 + y as f64;
                check_float(f, span).map(FormulaValue::Number)
            }
        }
        (x, y) => {
            let f = x.to_f64() + y.to_f64();
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

fn eval_sub(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) => {
            if let Some(res) = x.checked_sub(y) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = x as f64 - y as f64;
                check_float(f, span).map(FormulaValue::Number)
            }
        }
        (x, y) => {
            let f = x.to_f64() - y.to_f64();
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

fn eval_mul(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) => {
            if let Some(res) = x.checked_mul(y) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = x as f64 * y as f64;
                check_float(f, span).map(FormulaValue::Number)
            }
        }
        (x, y) => {
            let f = x.to_f64() * y.to_f64();
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

fn eval_div(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    if b.to_f64() == 0.0 {
        return Err(FormulaError::division_by_zero(span));
    }
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) if x % y == 0 => {
            if let Some(res) = x.checked_div(y) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = x as f64 / y as f64;
                check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
            }
        }
        (x, y) => {
            let f = x.to_f64() / y.to_f64();
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

fn eval_rem(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    if b.to_f64() == 0.0 {
        return Err(FormulaError::division_by_zero(span));
    }
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) => {
            if let Some(res) = x.checked_rem(y) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = (x as f64) % (y as f64);
                check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
            }
        }
        (x, y) => {
            let f = x.to_f64() % y.to_f64();
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

pub fn eval_pow(
    a: NumberValue,
    b: NumberValue,
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    match (a, b) {
        (NumberValue::Int(x), NumberValue::Int(y)) if y >= 0 && y <= u32::MAX as i64 => {
            if let Some(res) = x.checked_pow(y as u32) {
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let f = (x as f64).powf(y as f64);
                check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
            }
        }
        (x, y) => {
            let f = x.to_f64().powf(y.to_f64());
            check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

pub fn check_float(f: f64, span: Range<usize>) -> Result<NumberValue, FormulaError> {
    if f.is_nan() {
        Err(FormulaError::domain_error(span, "operation"))
    } else if f.is_infinite() {
        Err(FormulaError::overflow(span))
    } else {
        Ok(NumberValue::Float(f))
    }
}
