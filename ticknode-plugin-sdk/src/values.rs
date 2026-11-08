use std::collections::{BTreeMap, HashMap};
use crate::{DataValue, PinId, ObjectField};

/// Values of connected input data pins.
#[derive(Debug, Default, Clone)]
pub struct Inputs(pub(crate) HashMap<PinId, DataValue>);

impl Inputs {
    pub fn get(&self, pin: PinId) -> Option<&DataValue> {
        self.0.get(&pin)
    }

    pub fn number(&self, pin: PinId, default: f64) -> f64 {
        match self.get(pin) {
            Some(DataValue::Number(n)) => n.to_f64(),
            _ => default,
        }
    }

    /// Field of a struct value on input `pin`.
    pub fn field(&self, pin: PinId, name: &str) -> Option<&DataValue> {
        field(self.get(pin)?, name)
    }

    pub fn field_number(&self, pin: PinId, name: &str) -> Option<f64> {
        field_number(self.get(pin)?, name)
    }

    pub fn field_string(&self, pin: PinId, name: &str) -> Option<&str> {
        field_string(self.get(pin)?, name)
    }

    pub fn field_bool(&self, pin: PinId, name: &str) -> Option<bool> {
        field_bool(self.get(pin)?, name)
    }

    pub fn bool(&self, pin: PinId, default: bool) -> bool {
        match self.get(pin) {
            Some(DataValue::Bool(b)) => *b,
            _ => default,
        }
    }

    /// String value, or an empty string if the pin is unconnected or holds another type.
    pub fn string(&self, pin: PinId) -> String {
        match self.get(pin) {
            Some(DataValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

/// Builder for a custom struct value: `StructValue::new("ha.state").field("state", "on").build()`.
/// Fields absent from `TypeDefinition` are not checked by the host, so field names must match the declaration.
#[derive(Debug, Clone)]
pub struct StructValue {
    type_name: String,
    fields: BTreeMap<String, ObjectField>,
}

impl StructValue {
    pub fn new(type_name: &str) -> Self {
        Self { type_name: type_name.into(), fields: BTreeMap::new() }
    }

    pub fn field(mut self, name: &str, value: impl Into<DataValue>) -> Self {
        self.fields.insert(name.into(), ObjectField { name: name.into(), description: String::new(), value: value.into() });
        self
    }

    /// Adds an optional field only if `value` is `Some`.
    pub fn field_opt(self, name: &str, value: Option<impl Into<DataValue>>) -> Self {
        match value {
            Some(v) => self.field(name, v),
            None => self,
        }
    }

    pub fn build(self) -> DataValue {
        DataValue::Object { fields: self.fields, type_name: Some(self.type_name) }
    }
}

impl From<StructValue> for DataValue {
    fn from(v: StructValue) -> Self { v.build() }
}

/// Field of a struct value (or any object).
pub fn field<'a>(value: &'a DataValue, name: &str) -> Option<&'a DataValue> {
    match value {
        DataValue::Object { fields, .. } => fields.get(name).map(|f| &f.value),
        _ => None,
    }
}

pub fn field_number(value: &DataValue, name: &str) -> Option<f64> {
    match field(value, name)? {
        DataValue::Number(n) => Some(n.to_f64()),
        _ => None,
    }
}

pub fn field_string<'a>(value: &'a DataValue, name: &str) -> Option<&'a str> {
    match field(value, name)? {
        DataValue::String(s) => Some(s),
        _ => None,
    }
}

pub fn field_bool(value: &DataValue, name: &str) -> Option<bool> {
    match field(value, name)? {
        DataValue::Bool(b) => Some(*b),
        _ => None,
    }
}
