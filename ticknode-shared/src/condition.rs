//! Match node conditions in the style of Factorio's decider combinator. A node has explicit inputs
//! (`MatchInput`: pin, name, type) and cases (`MatchCase`): each case is a list of "input  operator  operand"
//! rows joined by "and" / "or" ("and" binds tighter: `a and b or c` = `(a and b) or c`).
//!
//! Operators depend on the left input's type (`ops_for`): comparisons for numbers and time,
//! "contains" / "starts with" for strings, "contains" / "length" for lists, etc. The right operand is another
//! input or a constant; a constant is coerced to the left value's type (`text::coerce_to`), so `"07:30"` is
//! compared against a time value as time. Unary operators ("is empty", "has value") have no right operand.
//!
//! Text representation (stored in node properties, human-readable in git):
//! input — `"1:a:number"`, case — `"<pin>: a > 10 and s contains \"on\" or l is_empty"`.

use crate::{DataType, DataValue};

mod display;
mod error;
mod evaluation;
mod lexer;
mod operators;
mod parsing;

pub use error::{ConditionError, Expected};
pub use evaluation::loosely_equal;
pub use operators::{CompareOp, ops_for, ops_in};

/// Condition operand: a node input or a constant.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Input(String),
    Value(DataValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// How this row joins with the previous one (ignored on the first row).
    pub join: Join,
    pub left: Operand,
    pub op: CompareOp,
    /// `None` for unary operators.
    pub right: Option<Operand>,
}

/// A single output of a Match node: an exec pin and the conditions under which it fires (empty = always).
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCase {
    pub pin: u32,
    pub conditions: Vec<Condition>,
}

impl MatchCase {
    /// Input names referenced by the conditions.
    pub fn inputs(&self) -> impl Iterator<Item = &str> {
        self.conditions
            .iter()
            .flat_map(|c| std::iter::once(&c.left).chain(c.right.as_ref()))
            .filter_map(|o| match o {
                Operand::Input(name) => Some(name.as_str()),
                Operand::Value(_) => None,
            })
    }

    /// Renames an input across all conditions.
    pub fn rename_input(&mut self, from: &str, to: &str) {
        for c in &mut self.conditions {
            for o in std::iter::once(&mut c.left).chain(c.right.as_mut()) {
                if matches!(o, Operand::Input(n) if n == from) {
                    *o = Operand::Input(to.to_string());
                }
            }
        }
    }
}

/// Explicit input of a Match node: an input data pin with a name referenced by conditions.
/// Type `Any` means "auto": the input accepts anything, and operators follow the actual connected type.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchInput {
    pub pin: u32,
    pub name: String,
    pub value_type: DataType,
}

/// Types selectable for a Match input (names match `input_type_name`).
pub const INPUT_TYPES: [&str; 10] = [
    "any", "number", "string", "bool", "time", "datetime", "duration", "color", "list", "object",
];

pub fn input_type_name(t: &DataType) -> String {
    match t {
        DataType::List(inner) if **inner == DataType::Any => "list".into(),
        other => crate::data_type_name(other),
    }
}

pub fn parse_input_type(s: &str) -> Option<DataType> {
    match s.trim() {
        "list" | "list<any>" => Some(DataType::List(Box::new(DataType::Any))),
        other => crate::parse_data_type(other),
    }
}

/// Inputs for legacy Match nodes (without an `inputs` property): sorted condition input names, type `Any`, pins `1..`.
pub fn match_inputs(cases: &[MatchCase]) -> Vec<String> {
    let mut names: Vec<String> = cases
        .iter()
        .flat_map(|c| c.inputs().map(str::to_string))
        .collect();
    names.sort();
    names.dedup();
    names
}

pub fn is_input_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_')
        && !matches!(s, "true" | "false" | "and" | "or" | "і" | "та" | "або")
        && CompareOp::parse(s).is_none()
}

#[cfg(test)]
mod tests;
