//! Plain Jinja values and separate typed input presentation.

use crate::NodeContext;
use minijinja::{Value, value::ValueKind};
use ticknode_shared::{
    quantity::QuantityLabels,
    to_text::{ToTextOptions, to_text_in},
    *,
};

pub(super) fn plain(value: &DataValue) -> Value {
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
        _ => Value::from(text::format_value(value)),
    }
}

/// Input presentation retains the source contract independently of the Jinja value.
pub(super) fn input_text(value: &DataValue, declared: &DataType, ctx: &NodeContext) -> Value {
    Value::from(to_text_in(
        value,
        declared,
        &ToTextOptions::default(),
        &ctx.host_state.display,
        crate::quantity::labels(&ctx.host_state.locale),
        ctx.types,
    ))
}

fn data_value(value: &Value) -> Option<DataValue> {
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

/// Runtime kinds use project presentation without inferring source types or units.
pub(super) fn text_filter(presentation: &Presentation, value: Value) -> String {
    match value.kind() {
        // Jinja arithmetic can produce integers larger than DataValue's i64 range.
        ValueKind::Number if value.is_integer() && value.as_i64().is_none() => {
            let number = value.to_string();
            let rule = presentation.display.number;
            if rule.rounding == Rounding::Decimals && rule.digits > 0 {
                return format!(
                    "{number}{}{}",
                    crate::quantity::labels(&presentation.locale).decimal_separator(),
                    "0".repeat(usize::from(rule.digits)),
                );
            }
            return number;
        }
        ValueKind::Seq => {
            return value
                .try_iter()
                .into_iter()
                .flatten()
                .map(|v| text_filter(presentation, v))
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
                        text_filter(presentation, v)
                    };
                    format!("{key}: {text}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            return format!("{{{fields}}}");
        }
        _ => {}
    }
    let Some(value) = data_value(&value) else {
        return String::new();
    };
    to_text_in(
        &value,
        &DataType::Any,
        &ToTextOptions::default(),
        &presentation.display,
        crate::quantity::labels(&presentation.locale),
        &TypeRegistry::new(),
    )
}

#[derive(Debug)]
pub(super) struct Presentation {
    display: DisplayRules,
    locale: String,
}

pub(super) fn presentation(ctx: &NodeContext) -> Presentation {
    Presentation {
        display: ctx.host_state.display.clone(),
        locale: ctx.host_state.locale.clone(),
    }
}
