use crate::{DataType, DataValue, ObjectField, PinId};
use std::collections::{BTreeMap, HashMap};

/// Values of connected input data pins.
#[derive(Debug, Default, Clone)]
pub struct Inputs(pub(crate) HashMap<PinId, DataValue>);

impl FromIterator<(PinId, DataValue)> for Inputs {
    fn from_iter<T: IntoIterator<Item = (PinId, DataValue)>>(values: T) -> Self {
        Self(values.into_iter().collect())
    }
}

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
        Self {
            type_name: type_name.into(),
            fields: BTreeMap::new(),
        }
    }

    pub fn field(mut self, name: &str, value: impl Into<DataValue>) -> Self {
        let value = value.into();
        let data_type = value.get_type();
        self = self.typed_field(name, data_type, value);
        self
    }

    /// Adds a field with its semantic contract (e.g. a quantity or opaque identifier).
    pub fn typed_field(
        mut self,
        name: &str,
        data_type: DataType,
        value: impl Into<DataValue>,
    ) -> Self {
        let mut value = value.into();
        ticknode_shared::retain_field_types(
            &mut value,
            &data_type,
            &ticknode_shared::TypeRegistry::new(),
        );
        self.fields.insert(
            name.into(),
            ObjectField {
                data_type: Some(data_type),
                value,
            },
        );
        self
    }

    /// Adds an optional field only if `value` is `Some`.
    pub fn field_opt(self, name: &str, value: Option<impl Into<DataValue>>) -> Self {
        match value {
            Some(v) => self.field(name, v),
            None => self,
        }
    }

    /// Adds an optional field while retaining its declared semantic contract.
    pub fn typed_field_opt(
        self,
        name: &str,
        data_type: DataType,
        value: Option<impl Into<DataValue>>,
    ) -> Self {
        match value {
            Some(value) => self.typed_field(name, data_type, value),
            None => self,
        }
    }

    pub fn build(self) -> DataValue {
        DataValue::Object {
            fields: self.fields,
            type_name: Some(self.type_name),
        }
    }
}

impl From<StructValue> for DataValue {
    fn from(v: StructValue) -> Self {
        v.build()
    }
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

#[cfg(test)]
mod typed_tests {
    use super::*;
    #[test]
    fn builder_keeps_explicit_quantity_and_identifier_list_contracts() {
        let quantity = DataType::Named("quantity.power".into());
        let ids = DataType::Named("test.id".into());
        let value = StructValue::new("test.device")
            .typed_field("power", quantity.clone(), 1200i64)
            .typed_field(
                "ids",
                DataType::List(Box::new(ids.clone())),
                DataValue::List {
                    element_type: DataType::Any,
                    elements: vec![42i64.into()],
                },
            )
            .field("name", "Kitchen")
            .build();
        let DataValue::Object { fields, .. } = &value else {
            panic!("not an object")
        };
        assert_eq!(fields["power"].data_type, Some(quantity));
        assert_eq!(fields["name"].data_type, Some(DataType::String));
        assert_eq!(
            fields["ids"].value.get_type(),
            DataType::List(Box::new(ids))
        );
        let bytes = postcard::to_allocvec(&value).unwrap();
        assert_eq!(postcard::from_bytes::<DataValue>(&bytes).unwrap(), value);
    }
}
