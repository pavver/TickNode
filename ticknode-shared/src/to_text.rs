//! Pure presentation shared by the To text node and quantity wire conversions.
use crate::{
    quantity::{self, QuantityFormat, QuantityLabels, format_number, format_quantity},
    *,
};
use chrono::{
    Local,
    format::{Item, StrftimeItems},
};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToTextOptions {
    pub show_units: bool,
    pub unit: Option<String>,
    pub digits: Option<u8>,
    pub true_text: String,
    pub false_text: String,
    pub datetime_format: String,
}

impl Default for ToTextOptions {
    fn default() -> Self {
        Self {
            show_units: true,
            unit: None,
            digits: None,
            true_text: String::new(),
            false_text: String::new(),
            datetime_format: String::new(),
        }
    }
}

pub fn quantity_text(
    value: f64,
    name: &str,
    kind: &quantity::QuantityKind,
    options: &ToTextOptions,
    rules: &DisplayRules,
    labels: &dyn QuantityLabels,
) -> String {
    let rule = rules.effective(name, kind);
    let format = QuantityFormat {
        unit: options.unit.clone(),
        digits: options.digits.unwrap_or(rule.digits),
    };
    if options.show_units {
        return format_quantity(value, kind, &format, &rule, labels);
    }
    let unit = format
        .unit
        .as_ref()
        .and_then(|id| kind.units.iter().find(|u| &u.id == id))
        .unwrap_or_else(|| rule.unit(value, kind, format.digits));
    format_number(quantity::from_canonical(value, unit), format.digits, labels)
}

pub fn to_text(
    value: &DataValue,
    declared: &DataType,
    options: &ToTextOptions,
    rules: &DisplayRules,
    labels: &dyn QuantityLabels,
) -> String {
    to_text_in(
        value,
        declared,
        options,
        rules,
        labels,
        &TypeRegistry::new(),
    )
}

/// The registry supplies declared field types when presenting plugin structures.
pub fn to_text_in(
    value: &DataValue,
    declared: &DataType,
    options: &ToTextOptions,
    rules: &DisplayRules,
    labels: &dyn QuantityLabels,
    types: &TypeRegistry,
) -> String {
    let runtime = value.get_type();
    let declared = if *declared == DataType::Any {
        &runtime
    } else {
        declared
    };
    if let (DataValue::Number(n), DataType::Named(name)) = (value, declared)
        && let Some(q) = quantity::kind(name)
    {
        return quantity_text(n.to_f64(), name, q, options, rules, labels);
    }
    let digits = options.digits.unwrap_or(3);
    let format = |default: &str| {
        let requested = if options.datetime_format.is_empty() {
            default
        } else {
            &options.datetime_format
        };
        if StrftimeItems::new(requested).any(|item| matches!(item, Item::Error)) {
            default.to_string()
        } else {
            requested.to_string()
        }
    };
    match value {
        DataValue::Number(NumberValue::Int(n)) => n.to_string(),
        DataValue::Number(n) => format_number(n.to_f64(), digits, labels),
        DataValue::String(s) => s.clone(),
        DataValue::Bool(b) => {
            let custom = if *b {
                &options.true_text
            } else {
                &options.false_text
            };
            if custom.is_empty() {
                labels.bool_text(*b)
            } else {
                custom.clone()
            }
        }
        DataValue::Time(t) => {
            let mut result = String::new();
            if write!(&mut result, "{}", t.format(&format("%H:%M"))).is_ok() {
                result
            } else {
                t.format("%H:%M").to_string()
            }
        }
        DataValue::DateTime(t) => {
            let t = t.with_timezone(&Local);
            let mut result = String::new();
            if write!(&mut result, "{}", t.format(&format("%Y-%m-%d %H:%M"))).is_ok() {
                result
            } else {
                t.format("%Y-%m-%d %H:%M").to_string()
            }
        }
        DataValue::Duration(d) if !options.show_units => format_number(d.as_secs_f64(), 15, labels),
        DataValue::Duration(d) => {
            // Reuse the canonical decomposition, replacing only the numeric separator and unit labels.
            text::format_duration(*d)
                .split_whitespace()
                .map(|part| {
                    let i = part
                        .find(|c: char| c.is_ascii_alphabetic())
                        .unwrap_or(part.len());
                    format!(
                        "{}{}",
                        part[..i].replace('.', &labels.decimal_separator()),
                        labels.duration_unit(&part[i..])
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        }
        DataValue::Ticks(_) | DataValue::Color(_) => text::format_value(value),
        DataValue::List {
            elements,
            element_type,
        } => {
            let element = match declared {
                DataType::List(t) => t.as_ref(),
                _ => element_type,
            };
            elements
                .iter()
                .map(|v| to_text_in(v, element, options, rules, labels, types))
                .collect::<Vec<_>>()
                .join(", ")
        }
        DataValue::Object { fields, .. } => {
            let definition = types.definition_of(declared);
            let shown = fields
                .iter()
                .map(|(name, field)| {
                    let ty = definition
                        .and_then(|d| d.fields().iter().find(|f| &f.name == name))
                        .map(|f| &f.data_type);
                    let text = to_text_in(
                        &field.value,
                        ty.unwrap_or(&DataType::Any),
                        options,
                        rules,
                        labels,
                        types,
                    );
                    let text = if matches!(field.value, DataValue::String(_)) {
                        quote(&text)
                    } else {
                        text
                    };
                    format!("{name}: {text}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{shown}}}")
        }
    }
}

fn quote(text: &str) -> String {
    let escaped: String = text
        .chars()
        .map(|c| match c {
            '\"' => "\\\"".into(),
            '\\' => "\\\\".into(),
            '\n' => "\\n".into(),
            '\r' => "\\r".into(),
            '\t' => "\\t".into(),
            c if c.is_control() => format!("\\u{:04x}", c as u32),
            c => c.to_string(),
        })
        .collect();
    format!("\"{escaped}\"")
}

/// Properties remain persisted even while their controls are hidden.
pub fn property_visible(t: &DataType, name: &str) -> bool {
    let mut element = t;
    while let DataType::List(inner) = element {
        element = inner;
    }
    let quantity = matches!(element, DataType::Named(n) if quantity::kind(n).is_some());
    match name {
        "value_type" | "locale" => true,
        "unit" => quantity,
        "show_units" => quantity || *element == DataType::Duration,
        "digits" => quantity || *element == DataType::Number,
        "true_text" | "false_text" => *element == DataType::Bool,
        "datetime_format" => matches!(element, DataType::DateTime | DataType::Time),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
