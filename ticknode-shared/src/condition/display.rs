use super::{CompareOp, Condition, Join, MatchCase, MatchInput, Operand, input_type_name};
use crate::text::format_value;
use crate::{DataType, DataValue, NumberValue};
use CompareOp::*;

impl CompareOp {
    /// Storage (and input) representation.
    pub fn text(self) -> &'static str {
        self.presentation().0
    }

    /// Display symbol in a condition on a node.
    pub fn symbol(self) -> &'static str {
        self.presentation().1
    }

    fn presentation(self) -> (&'static str, &'static str) {
        const FORMS: &[(CompareOp, &str, &str)] = &[
            (Eq, "=", "="),
            (Ne, "!=", "≠"),
            (Lt, "<", "<"),
            (Le, "<=", "≤"),
            (Gt, ">", ">"),
            (Ge, ">=", "≥"),
            (Contains, "contains", "contains"),
            (NotContains, "not_contains", "not contains"),
            (StartsWith, "starts_with", "starts with"),
            (EndsWith, "ends_with", "ends with"),
            (IsEmpty, "is_empty", "is empty"),
            (NotEmpty, "not_empty", "is not empty"),
            (IsTrue, "is_true", "= true"),
            (IsFalse, "is_false", "= false"),
            (HasField, "has_field", "has field"),
            (NoField, "no_field", "no field"),
            (LenEq, "len_eq", "length ="),
            (LenGt, "len_gt", "length >"),
            (LenLt, "len_lt", "length <"),
            (HasValue, "has_value", "has value"),
            (NoValue, "no_value", "no value"),
        ];
        let (_, text, symbol) = FORMS
            .iter()
            .find(|(op, _, _)| *op == self)
            .expect("every operator has a presentation");
        (text, symbol)
    }

    /// Label in the operator selector — type-aware (for time, `<` means "earlier").
    pub fn label_for(self, t: &DataType) -> String {
        let time = matches!(t, DataType::Time | DataType::DateTime);
        match self {
            Lt if time => "< earlier".into(),
            Le if time => "≤ not later".into(),
            Gt if time => "> later".into(),
            Ge if time => "≥ not earlier".into(),
            Lt if *t == DataType::Duration => "< shorter".into(),
            Gt if *t == DataType::Duration => "> longer".into(),
            op => op.symbol().into(),
        }
    }
}

impl Operand {
    pub fn to_text(&self) -> String {
        match self {
            Self::Input(name) => name.clone(),
            Self::Value(DataValue::String(s)) => quote(s),
            Self::Value(DataValue::Number(NumberValue::Int(i))) => i.to_string(),
            Self::Value(DataValue::Number(NumberValue::Float(f))) => f.to_string(),
            Self::Value(DataValue::Bool(b)) => b.to_string(),
            // Time, duration, color, etc. are formatted as strings and coerced back to the input type on comparison
            Self::Value(other) => quote(&format_value(other)),
        }
    }
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl Join {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::And => "and",
            Self::Or => "or",
        }
    }
    fn text(self) -> &'static str {
        self.symbol()
    }
}

impl Condition {
    fn to_text(
        &self,
        op: impl Fn(CompareOp) -> String,
        value: impl Fn(&Operand) -> String,
    ) -> String {
        match &self.right {
            Some(r) => format!("{} {} {}", value(&self.left), op(self.op), value(r)),
            None => format!("{} {}", value(&self.left), op(self.op)),
        }
    }
}

impl MatchCase {
    pub fn to_text(&self) -> String {
        let body = self
            .conditions
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let cond = c.to_text(|op| op.text().to_string(), Operand::to_text);
                if i == 0 {
                    cond
                } else {
                    format!("{} {}", c.join.text(), cond)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!("{}: {}", self.pin, body).trim_end().to_string()
    }

    /// Human-readable display format using localized operator and join symbols.
    pub fn display(&self) -> String {
        if self.conditions.is_empty() {
            return "always".into();
        }
        self.conditions
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let cond = c.to_text(|op| op.symbol().to_string(), Operand::to_text);
                if i == 0 {
                    cond
                } else {
                    format!("{} {}", c.join.symbol(), cond)
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl MatchInput {
    pub fn to_text(&self) -> String {
        format!(
            "{}:{}:{}",
            self.pin,
            self.name,
            input_type_name(&self.value_type)
        )
    }
}
