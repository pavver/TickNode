use serde::{Deserialize, Serialize};
use chrono::{NaiveTime, DateTime, Utc};
use std::time::Duration;
use std::ops::{Add, Sub, Mul, Div};
use std::collections::BTreeMap;
use crate::{TypeRegistry, is_type_name};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NumberValue {
    Int(i64),
    Float(f64),
}

impl NumberValue {
    pub fn to_f64(self) -> f64 {
        match self {
            Self::Int(v) => v as f64,
            Self::Float(v) => v,
        }
    }

    pub fn normalize(self) -> Self {
        match self {
            Self::Float(f) => {
                if f.fract() == 0.0 && f <= i64::MAX as f64 && f >= i64::MIN as f64 {
                    Self::Int(f as i64)
                } else {
                    Self::Float(f)
                }
            }
            Self::Int(i) => Self::Int(i),
        }
    }
}

impl Add for NumberValue {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a + b),
            (a, b) => Self::Float(a.to_f64() + b.to_f64()).normalize(),
        }
    }
}

impl Sub for NumberValue {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a - b),
            (a, b) => Self::Float(a.to_f64() - b.to_f64()).normalize(),
        }
    }
}

impl Mul for NumberValue {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a * b),
            (a, b) => Self::Float(a.to_f64() * b.to_f64()).normalize(),
        }
    }
}

impl Div for NumberValue {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::Float(self.to_f64() / rhs.to_f64()).normalize()
    }
}

impl From<i64> for NumberValue {
    fn from(v: i64) -> Self { Self::Int(v) }
}

impl From<f64> for NumberValue {
    fn from(v: f64) -> Self { Self::Float(v) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataType {
    Bool,
    Number,
    String,
    Time,
    DateTime,
    Duration,
    Color,
    List(Box<DataType>),
    Object,
    Any,
    /// Plugin-declared type by full name (`ha.state`); definition lives in `TypeRegistry`.
    Named(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectField {
    pub name: String,
    pub description: String,
    pub value: DataValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataValue {
    Bool(bool),
    Number(NumberValue),
    String(String),
    Time(NaiveTime),
    DateTime(DateTime<Utc>),
    Duration(Duration),
    Color(Color),
    List {
        element_type: DataType,
        elements: Vec<DataValue>,
    },
    Object {
        fields: BTreeMap<String, ObjectField>,
        /// Name of a custom struct type; `None` for an untyped object.
        /// Without `skip_serializing_if`: postcard is not self-describing.
        #[serde(default)]
        type_name: Option<String>,
    },
}

impl DataValue {
    pub fn get_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::Number(_) => DataType::Number,
            Self::String(_) => DataType::String,
            Self::Time(_) => DataType::Time,
            Self::DateTime(_) => DataType::DateTime,
            Self::Duration(_) => DataType::Duration,
            Self::Color(_) => DataType::Color,
            Self::List { element_type, .. } => DataType::List(Box::new(element_type.clone())),
            Self::Object { type_name: Some(name), .. } => DataType::Named(name.clone()),
            Self::Object { .. } => DataType::Object,
        }
    }
}

impl From<bool> for DataValue { fn from(v: bool) -> Self { Self::Bool(v) } }
impl From<f64> for DataValue { fn from(v: f64) -> Self { Self::Number(NumberValue::Float(v).normalize()) } }
impl From<i64> for DataValue { fn from(v: i64) -> Self { Self::Number(NumberValue::Int(v)) } }
impl From<&str> for DataValue { fn from(v: &str) -> Self { Self::String(v.into()) } }
impl From<String> for DataValue { fn from(v: String) -> Self { Self::String(v) } }

/// Whether an output of type `src` can be connected to an input of type `dst` when there are no custom types:
/// equal, or at least one is `Any` (also inside lists: `list<number>` → `list<any>`).
/// With custom types, use `TypeRegistry::compatible`.
pub fn types_compatible(src: &DataType, dst: &DataType) -> bool {
    TypeRegistry::default().compatible(src, dst)
}

const BUILTIN_TYPES: &[(DataType, &str)] = &[
    (DataType::Bool, "bool"), (DataType::Number, "number"), (DataType::String, "string"),
    (DataType::Time, "time"), (DataType::DateTime, "datetime"), (DataType::Duration, "duration"),
    (DataType::Color, "color"), (DataType::Object, "object"), (DataType::Any, "any"),
];

pub fn parse_data_type(s: &str) -> Option<DataType> {
    let lower = s.trim().to_lowercase();
    if let Some((ty, _)) = BUILTIN_TYPES.iter().find(|(_, name)| *name == lower) {
        return Some(ty.clone());
    }
    if let Some(inner) = lower.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
        return parse_data_type(inner).map(|t| DataType::List(Box::new(t)));
    }
    // Custom type: case is preserved (type names are lowercase, but we do not guess)
    let name = s.trim();
    is_type_name(name).then(|| DataType::Named(name.to_string()))
}

pub fn data_type_name(t: &DataType) -> String {
    match t {
        DataType::Named(name) => name.clone(),
        DataType::List(inner) => format!("list<{}>", data_type_name(inner)),
        _ => BUILTIN_TYPES.iter().find(|(ty, _)| ty == t).expect("every built-in type has a name").1.into(),
    }
}
