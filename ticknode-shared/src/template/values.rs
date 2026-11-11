//! Value conversion and runtime formatting for templates.

use minijinja::{Value, value::ValueKind};
use crate::{DataValue, NumberValue};

pub fn plain(value: &DataValue) -> Value {
    match value {
        DataValue::Number(NumberValue::Int(n)) => Value::from(*n),
        DataValue::Number(NumberValue::Float(n)) => Value::from(*n),
        DataValue::Bool(b) => Value::from(*b),
        DataValue::String(s) => Value::from(s.clone()),
        DataValue::List { elements, .. } => elements.iter().map(plain).collect(),
        DataValue::Object { fields, .. } => fields
            .iter()
            .map(|(name, field)| (name.clone(), plain(&field.value)))
            .collect(),
        DataValue::DateTime(t) => Value::from(t.to_rfc3339()),
        _ => Value::from(crate::text::format_value(value)),
    }
}

pub(super) fn data_value(value: &Value) -> Option<DataValue> {
    Some(match value.kind() {
        ValueKind::Bool => DataValue::Bool(value.is_true()),
        ValueKind::Number if value.is_integer() => {
            DataValue::Number(NumberValue::Int(value.as_i64()?))
        }
        ValueKind::Number => {
            DataValue::Number(NumberValue::Float(f64::try_from(value.clone()).ok()?))
        }
        ValueKind::String => DataValue::String(value.to_string()),
        _ => return None,
    })
}

pub(super) fn format_runtime_value(
    value: &Value,
    runtime_text: &dyn Fn(&DataValue) -> String,
    int_decimal_suffix: &str,
) -> String {
    match value.kind() {
        ValueKind::Number if value.is_integer() && value.as_i64().is_none() => {
            return format!("{}{int_decimal_suffix}", value);
        }
        ValueKind::Seq => {
            return value
                .try_iter()
                .into_iter()
                .flatten()
                .map(|v| format_runtime_value(&v, runtime_text, int_decimal_suffix))
                .collect::<Vec<_>>()
                .join(", ");
        }
        ValueKind::Map => {
            let fields = value
                .try_iter()
                .into_iter()
                .flatten()
                .map(|key| {
                    let v = value.get_item(&key).unwrap_or(Value::UNDEFINED);
                    let text = if v.kind() == ValueKind::String {
                        format!("{v:?}")
                    } else {
                        format_runtime_value(&v, runtime_text, int_decimal_suffix)
                    };
                    format!("{key}: {text}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{{{fields}}}");
        }
        _ => {}
    }
    let Some(value) = data_value(value) else {
        return String::new();
    };
    runtime_text(&value)
}
