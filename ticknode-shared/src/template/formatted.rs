//! Formatted trees retain key/index access and render containers with To text presentation.
use crate::{DataType, DataValue, TypeRegistry, effective_type};
use minijinja::value::{Enumerator, Object, ObjectRepr, Value};
use std::{collections::HashMap, fmt, sync::Arc};

#[derive(Debug)]
struct Formatted {
    text: String,
    children: Value,
    sequence: bool,
}
impl Object for Formatted {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        if self.sequence {
            ObjectRepr::Seq
        } else {
            ObjectRepr::Map
        }
    }
    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.children
            .get_item(key)
            .ok()
            .filter(|v| !v.is_undefined())
    }
    fn enumerate(self: &Arc<Self>) -> Enumerator {
        match self.children.try_iter() {
            Ok(iter) => Enumerator::Values(iter.collect()),
            Err(_) => Enumerator::Empty,
        }
    }
    fn render(self: &Arc<Self>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

pub(super) fn value(
    value: &DataValue,
    path: &str,
    path_types: &HashMap<String, DataType>,
    types: &TypeRegistry,
    format: &dyn Fn(&DataValue, &DataType) -> String,
) -> Value {
    let declared = path_types.get(path).unwrap_or(&DataType::Any);
    let ty = effective_type(Some(value), declared, "", types).unwrap_or_else(|| declared.clone());
    let text = format(value, &ty);
    let (children, sequence) = match value {
        DataValue::Object { fields, .. } => (
            fields
                .iter()
                .map(|(key, field)| {
                    (
                        key.clone(),
                        self::value(
                            &field.value,
                            &format!("{path}.{key}"),
                            path_types,
                            types,
                            format,
                        ),
                    )
                })
                .collect(),
            false,
        ),
        DataValue::List { elements, .. } => (
            elements
                .iter()
                .enumerate()
                .map(|(i, v)| self::value(v, &format!("{path}.{i}"), path_types, types, format))
                .collect(),
            true,
        ),
        _ => return Value::from(text),
    };
    Value::from_object(Formatted {
        text,
        children,
        sequence,
    })
}
