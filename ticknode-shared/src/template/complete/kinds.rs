use crate::{DataType, TypeRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValueKind {
    String,
    Number,
    Bool,
    List(Box<ValueKind>),
    Map(Option<String> /* struct type name */),
    Any,
}

impl ValueKind {
    pub fn is_iterable(&self) -> bool {
        matches!(
            self,
            ValueKind::List(_) | ValueKind::Map(_) | ValueKind::String | ValueKind::Any
        )
    }
}

pub fn kind_of(data_type: &DataType, types: &TypeRegistry) -> ValueKind {
    match data_type {
        DataType::Number => ValueKind::Number,
        DataType::String
        | DataType::Time
        | DataType::DateTime
        | DataType::Duration
        | DataType::Color => ValueKind::String,
        DataType::Bool => ValueKind::Bool,
        DataType::List(inner) => ValueKind::List(Box::new(kind_of(inner, types))),
        DataType::Object => ValueKind::Map(None),
        DataType::Any => ValueKind::Any,
        DataType::Named(name) => {
            if let Some(def) = types
                .get(name)
                .or_else(|| crate::quantity::catalog().iter().find(|d| d.name == *name))
            {
                if def.is_struct() {
                    ValueKind::Map(Some(name.clone()))
                } else if def.is_enum() {
                    ValueKind::String
                } else if def.is_identifier() || def.quantity().is_some() {
                    ValueKind::Number
                } else {
                    ValueKind::Any
                }
            } else {
                ValueKind::Any
            }
        }
    }
}

pub fn kind_label(kind: &ValueKind, types: &TypeRegistry) -> String {
    match kind {
        ValueKind::String => "String".to_string(),
        ValueKind::Number => "Number".to_string(),
        ValueKind::Bool => "Bool".to_string(),
        ValueKind::List(inner) => format!("list of {}", kind_label(inner, types)),
        ValueKind::Map(Some(name)) => {
            if let Some(def) = types.get(name) {
                def.title.clone()
            } else {
                name.clone()
            }
        }
        ValueKind::Map(None) => "map".to_string(),
        ValueKind::Any => "Any".to_string(),
    }
}
