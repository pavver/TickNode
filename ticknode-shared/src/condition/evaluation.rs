use std::cmp::Ordering;
use crate::{DataType, DataValue};
use crate::text::{coerce_to, format_value};
use super::{CompareOp, Condition, Operand, MatchCase, Join};
use CompareOp::*;

impl CompareOp {
    fn holds(self, ord: Ordering) -> bool {
        match self {
            Eq | LenEq => ord == Ordering::Equal,
            Ne => ord != Ordering::Equal,
            Lt | LenLt => ord == Ordering::Less,
            Le => ord != Ordering::Greater,
            Gt | LenGt => ord == Ordering::Greater,
            Ge => ord != Ordering::Less,
            _ => false,
        }
    }
}

impl Operand {
    fn resolve(&self, value_of: &impl Fn(&str) -> Option<DataValue>) -> Option<DataValue> {
        match self {
            Self::Input(name) => value_of(name),
            Self::Value(v) => Some(v.clone()),
        }
    }
}

impl Condition {
    /// An input without a value (unconnected and without an inline value on the node) does not satisfy
    /// the condition (except `NoValue`). An operator inapplicable to the value's type also evaluates to false.
    pub fn holds(&self, value_of: &impl Fn(&str) -> Option<DataValue>) -> bool {
        let a = self.left.resolve(value_of);
        match self.op {
            HasValue => return a.is_some(),
            NoValue => return a.is_none(),
            _ => {}
        }
        let Some(a) = a else { return false };
        if self.op.is_unary() {
            return unary(self.op, &a);
        }
        match self.right.as_ref().and_then(|r| r.resolve(value_of)) {
            Some(b) => binary(self.op, &a, b),
            None => false,
        }
    }
}

fn length(v: &DataValue) -> Option<usize> {
    match v {
        DataValue::String(s) => Some(s.chars().count()),
        DataValue::List { elements, .. } => Some(elements.len()),
        DataValue::Object { fields, .. } => Some(fields.len()),
        _ => None,
    }
}

fn unary(op: CompareOp, a: &DataValue) -> bool {
    match op {
        IsEmpty => length(a) == Some(0),
        NotEmpty => length(a).is_some_and(|n| n > 0),
        IsTrue => *a == DataValue::Bool(true),
        IsFalse => *a == DataValue::Bool(false),
        _ => false,
    }
}

/// Ordering for values of the same type (Int/Float numbers are compared together).
fn ordering(a: &DataValue, b: &DataValue) -> Option<Ordering> {
    match (a, b) {
        (DataValue::Number(x), DataValue::Number(y)) => x.to_f64().partial_cmp(&y.to_f64()),
        (DataValue::String(x), DataValue::String(y)) => Some(x.cmp(y)),
        (DataValue::Bool(x), DataValue::Bool(y)) => Some(x.cmp(y)),
        (DataValue::Time(x), DataValue::Time(y)) => Some(x.cmp(y)),
        (DataValue::DateTime(x), DataValue::DateTime(y)) => Some(x.cmp(y)),
        (DataValue::Duration(x), DataValue::Duration(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

/// Equality after coercing `b` to the type of `a` (`1` = `1.0`, `"07:30"` = time 07:30).
pub fn loosely_equal(a: &DataValue, b: &DataValue) -> bool {
    let b = coerce_to(b.clone(), &a.get_type());
    match ordering(a, &b) {
        Some(ord) => ord == Ordering::Equal,
        None => *a == b,
    }
}

fn binary(op: CompareOp, a: &DataValue, b: DataValue) -> bool {
    match op {
        Eq => loosely_equal(a, &b),
        Ne => !loosely_equal(a, &b),
        Lt | Le | Gt | Ge => ordering(a, &coerce_to(b, &a.get_type())).is_some_and(|ord| op.holds(ord)),
        Contains | NotContains => {
            let found = contains(a, &b);
            found.is_some_and(|f| f == (op == Contains))
        }
        StartsWith | EndsWith => string_boundary(op, a, &b),
        HasField | NoField => has_field(a, &b).is_some_and(|found| found == (op == HasField)),
        LenEq | LenGt | LenLt => compare_length(op, a, b),
        _ => false,
    }
}

fn string_boundary(op: CompareOp, a: &DataValue, b: &DataValue) -> bool {
    let DataValue::String(s) = a else { return false };
    match op {
        StartsWith => s.starts_with(&format_value(b)),
        EndsWith => s.ends_with(&format_value(b)),
        _ => false,
    }
}

fn contains(a: &DataValue, b: &DataValue) -> Option<bool> {
    match a {
        DataValue::String(s) => Some(s.contains(&format_value(b))),
        DataValue::List { elements, .. } => Some(elements.iter().any(|e| loosely_equal(e, b))),
        DataValue::Object { fields, .. } => Some(fields.contains_key(&format_value(b))),
        _ => None,
    }
}

fn has_field(a: &DataValue, b: &DataValue) -> Option<bool> {
    match a {
        DataValue::Object { fields, .. } => Some(fields.contains_key(&format_value(b))),
        _ => None,
    }
}

fn compare_length(op: CompareOp, a: &DataValue, b: DataValue) -> bool {
    match (length(a), coerce_to(b, &DataType::Number)) {
        (Some(n), DataValue::Number(m)) => (n as f64).partial_cmp(&m.to_f64()).is_some_and(|ord| op.holds(ord)),
        _ => false,
    }
}

impl MatchCase {
    /// Disjunction ("or") of conjunction ("and") groups; empty conditions always match.
    pub fn matches(&self, value_of: impl Fn(&str) -> Option<DataValue>) -> bool {
        if self.conditions.is_empty() {
            return true;
        }
        let mut any = false;
        let mut group = true;
        for (i, c) in self.conditions.iter().enumerate() {
            if i > 0 && c.join == Join::Or {
                any |= group;
                group = true;
            }
            group = group && c.holds(&value_of);
        }
        any || group
    }
}
