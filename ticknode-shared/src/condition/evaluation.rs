use super::{CompareOp, Condition, Join, MatchCase, Operand};
use crate::text::{coerce_to, format_value};
use crate::{DataType, DataValue};
use CompareOp::*;
use std::cmp::Ordering;

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
    /// Tick and wall-clock durations cannot be compared: all equality/ordering operators, including `Ne`, return false.
    pub fn holds(&self, value_of: &impl Fn(&str) -> Option<DataValue>) -> bool {
        let a = self.left.resolve(value_of);
        // Resolve the right operand only for binary operators with a present left value.
        let operands = || {
            let a = a.as_ref()?;
            let b = self.right.as_ref()?.resolve(value_of)?;
            Some((a, b))
        };
        let ordered = || operands().and_then(|(a, b)| ordering(a, &coerce_to(b, &a.get_type())));
        let length_order = || operands().and_then(|(a, b)| length_ordering(a, b));
        match self.op {
            HasValue => a.is_some(),
            NoValue => a.is_none(),
            IsEmpty => a.as_ref().and_then(length) == Some(0),
            NotEmpty => a.as_ref().and_then(length).is_some_and(|n| n > 0),
            IsTrue => a == Some(DataValue::Bool(true)),
            IsFalse => a == Some(DataValue::Bool(false)),
            Eq => operands().is_some_and(|(a, b)| loosely_equal(a, &b)),
            Ne => operands().is_some_and(|(a, b)| {
                let b = coerce_to(b, &a.get_type());
                !mixed_durations(a, &b) && !loosely_equal(a, &b)
            }),
            Lt => ordered() == Some(Ordering::Less),
            Le => ordered().is_some_and(|ord| ord != Ordering::Greater),
            Gt => ordered() == Some(Ordering::Greater),
            Ge => ordered().is_some_and(|ord| ord != Ordering::Less),
            Contains | NotContains => operands().is_some_and(|(a, b)| {
                contains(a, &b).is_some_and(|found| found == (self.op == Contains))
            }),
            StartsWith => operands().is_some_and(
                |(a, b)| matches!(a, DataValue::String(s) if s.starts_with(&format_value(&b))),
            ),
            EndsWith => operands().is_some_and(
                |(a, b)| matches!(a, DataValue::String(s) if s.ends_with(&format_value(&b))),
            ),
            HasField | NoField => operands().is_some_and(|(a, b)| {
                has_field(a, &b).is_some_and(|found| found == (self.op == HasField))
            }),
            LenEq => length_order() == Some(Ordering::Equal),
            LenGt => length_order() == Some(Ordering::Greater),
            LenLt => length_order() == Some(Ordering::Less),
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

/// Ordering for values of the same type (Int/Float numbers are compared together).
fn ordering(a: &DataValue, b: &DataValue) -> Option<Ordering> {
    match (a, b) {
        (DataValue::Number(crate::NumberValue::Int(x)), DataValue::Number(crate::NumberValue::Int(y))) => Some(x.cmp(y)),
        (DataValue::Number(x), DataValue::Number(y)) => x.to_f64().partial_cmp(&y.to_f64()),
        (DataValue::String(x), DataValue::String(y)) => Some(x.cmp(y)),
        (DataValue::Bool(x), DataValue::Bool(y)) => Some(x.cmp(y)),
        (DataValue::Time(x), DataValue::Time(y)) => Some(x.cmp(y)),
        (DataValue::DateTime(x), DataValue::DateTime(y)) => Some(x.cmp(y)),
        (DataValue::Duration(x), DataValue::Duration(y)) => Some(x.cmp(y)),
        (DataValue::Ticks(x), DataValue::Ticks(y)) => Some(x.cmp(y)),
        _ => None,
    }
}

fn mixed_durations(a: &DataValue, b: &DataValue) -> bool {
    matches!(
        (a, b),
        (DataValue::Ticks(_), DataValue::Duration(_))
            | (DataValue::Duration(_), DataValue::Ticks(_))
    )
}

/// Equality after coercing `b` to the type of `a` (`1` = `1.0`, `"07:30"` = time 07:30).
pub fn loosely_equal(a: &DataValue, b: &DataValue) -> bool {
    let b = coerce_to(b.clone(), &a.get_type());
    match ordering(a, &b) {
        Some(ord) => ord == Ordering::Equal,
        None => *a == b,
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

fn length_ordering(a: &DataValue, b: DataValue) -> Option<Ordering> {
    let n = length(a)?;
    let DataValue::Number(m) = coerce_to(b, &DataType::Number) else {
        return None;
    };
    (n as f64).partial_cmp(&m.to_f64())
}

impl MatchCase {
    /// Evaluates constants using declared types; different quantities never compare directly.
    pub fn matches_in(
        &self,
        types: &crate::TypeRegistry,
        type_of: impl Fn(&str) -> DataType,
        value_of: impl Fn(&str) -> Option<DataValue>,
    ) -> bool {
        self.matches_by(|condition| {
            let mut c = condition.clone();
            if let (Operand::Input(left), Some(Operand::Input(right))) = (&c.left, &c.right) {
                let a = type_of(left);
                let b = type_of(right);
                let quantity = |t: &DataType| {
                    types
                        .definition_of(t)
                        .is_some_and(|d| d.quantity().is_some())
                };
                let identifier = |t: &DataType| types.definition_of(t).is_some_and(|d| d.is_identifier());
                if ((quantity(&a) && quantity(&b)) || identifier(&a) || identifier(&b)) && !types.compatible(&a, &b) {
                    return false;
                }
            }
            if let Operand::Input(name) = &c.left
                && let Some(Operand::Value(v)) = &mut c.right
                && let Some(t) = c.op.right_type(&type_of(name))
            {
                if let DataValue::String(text) = v
                    && types
                        .definition_of(&t)
                        .is_some_and(|d| d.quantity().is_some())
                {
                    let Ok(parsed) = crate::text::parse_value_in(types, text, &t) else {
                        return false;
                    };
                    *v = parsed;
                } else {
                    *v = crate::text::coerce_to_in(types, v.clone(), &t);
                }
            }
            if let Operand::Input(name) = &c.left {
                let t = type_of(name);
                if types.definition_of(&t).is_some_and(|d| d.is_identifier()) {
                    if !super::ops_in(types, &t).contains(&c.op) { return false; }
                    if let Some(Operand::Value(v)) = &c.right
                        && !matches!(v, DataValue::Number(crate::NumberValue::Int(_))) { return false; }
                }
            }
            c.holds(&value_of)
        })
    }

    /// Disjunction ("or") of conjunction ("and") groups; empty conditions always match.
    pub fn matches(&self, value_of: impl Fn(&str) -> Option<DataValue>) -> bool {
        self.matches_by(|c| c.holds(&value_of))
    }

    fn matches_by(&self, holds: impl Fn(&Condition) -> bool) -> bool {
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
            group = group && holds(c);
        }
        any || group
    }
}
