use super::CompareOp::*;
use crate::DataType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    IsEmpty,
    NotEmpty,
    IsTrue,
    IsFalse,
    HasField,
    NoField,
    LenEq,
    LenGt,
    LenLt,
    HasValue,
    NoValue,
}

impl CompareOp {
    pub const ALL: [CompareOp; 21] = [
        Eq,
        Ne,
        Lt,
        Le,
        Gt,
        Ge,
        Contains,
        NotContains,
        StartsWith,
        EndsWith,
        IsEmpty,
        NotEmpty,
        IsTrue,
        IsFalse,
        HasField,
        NoField,
        LenEq,
        LenGt,
        LenLt,
        HasValue,
        NoValue,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "=" | "==" => Eq,
            "!=" | "≠" | "<>" => Ne,
            "<" => Lt,
            "<=" | "≤" => Le,
            ">" => Gt,
            ">=" | "≥" => Ge,
            _ => return CompareOp::ALL.into_iter().find(|op| op.text() == s),
        })
    }

    /// Whether the operator has no right operand.
    pub fn is_unary(self) -> bool {
        matches!(
            self,
            IsEmpty | NotEmpty | IsTrue | IsFalse | HasValue | NoValue
        )
    }

    /// Type of the right operand for a left operand of type `left` (`None` if the operator is unary).
    pub fn right_type(self, left: &DataType) -> Option<DataType> {
        if self.is_unary() {
            return None;
        }
        Some(match self {
            Contains | NotContains => match left {
                DataType::List(inner) => (**inner).clone(),
                DataType::Any => DataType::Any,
                _ => DataType::String,
            },
            StartsWith | EndsWith | HasField | NoField => DataType::String,
            LenEq | LenGt | LenLt => DataType::Number,
            _ => left.clone(),
        })
    }
}

/// Operators applicable to a left input of this type (in display order).
pub fn ops_for(t: &DataType) -> &'static [CompareOp] {
    match t {
        DataType::Number | DataType::Time | DataType::DateTime | DataType::Duration => {
            &[Eq, Ne, Lt, Le, Gt, Ge, HasValue, NoValue]
        }
        DataType::String => &[
            Eq,
            Ne,
            Contains,
            NotContains,
            StartsWith,
            EndsWith,
            IsEmpty,
            NotEmpty,
            LenEq,
            LenGt,
            LenLt,
            HasValue,
            NoValue,
        ],
        DataType::Bool => &[IsTrue, IsFalse, Eq, Ne, HasValue, NoValue],
        DataType::Color => &[Eq, Ne, HasValue, NoValue],
        DataType::List(_) => &[
            Contains,
            NotContains,
            IsEmpty,
            NotEmpty,
            LenEq,
            LenGt,
            LenLt,
            Eq,
            Ne,
            HasValue,
            NoValue,
        ],
        DataType::Object => &[
            HasField, NoField, IsEmpty, NotEmpty, Eq, Ne, HasValue, NoValue,
        ],
        // Custom type without a registry (enums; structs go through `ops_in`): equality and presence only
        DataType::Named(_) => &[Eq, Ne, HasValue, NoValue],
        DataType::Any => &[
            Eq,
            Ne,
            Lt,
            Le,
            Gt,
            Ge,
            Contains,
            NotContains,
            IsEmpty,
            NotEmpty,
            IsTrue,
            IsFalse,
            HasValue,
            NoValue,
        ],
    }
}

/// Like `ops_for`, but a struct from the registry behaves like an object (`has_field`, `is_empty`, etc.).
pub fn ops_in(types: &crate::TypeRegistry, t: &DataType) -> &'static [CompareOp] {
    match types.definition_of(t) {
        Some(def) if def.quantity().is_some() => ops_for(&DataType::Number),
        Some(def) if def.is_struct() => ops_for(&DataType::Object),
        _ => ops_for(t),
    }
}
