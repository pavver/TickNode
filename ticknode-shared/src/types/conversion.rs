use crate::{DataType, DataValue, NumberValue, text};

/// A conversion applied while pulling a connected input. Failure means no input value.
#[derive(Debug, Clone, PartialEq)]
pub enum Conversion {
    Parse(DataType),
    Format(DataType),
    StringToBool,
    NumberToBool,
    BoolToNumber,
    StringToEnum(Vec<String>),
    List {
        element_type: DataType,
        conversion: Box<Conversion>,
    },
    CanonicalNumber,
    ParseQuantity(crate::quantity::QuantityKind),
    FormatQuantity {
        name: String,
        kind: crate::quantity::QuantityKind,
    },
}

impl Conversion {
    pub fn apply(&self, value: DataValue) -> Option<DataValue> {
        self.apply_with_labels(value, &crate::quantity::CanonicalLabels)
    }

    pub fn apply_with_labels(
        &self,
        value: DataValue,
        labels: &dyn crate::quantity::QuantityLabels,
    ) -> Option<DataValue> {
        self.apply_with_display(value, &Default::default(), labels)
    }

    pub fn apply_with_display(
        &self,
        value: DataValue,
        rules: &crate::DisplayRules,
        labels: &dyn crate::quantity::QuantityLabels,
    ) -> Option<DataValue> {
        match (self, value) {
            (Self::CanonicalNumber, DataValue::Number(n)) if n.to_f64().is_finite() => {
                Some(DataValue::Number(n))
            }
            (Self::ParseQuantity(q), DataValue::String(s)) => {
                crate::quantity::parse_quantity(&s, q).map(DataValue::from)
            }
            (Self::FormatQuantity { name, kind: q }, DataValue::Number(n)) => {
                Some(DataValue::String(crate::to_text::quantity_text(
                    n.to_f64(),
                    name,
                    q,
                    &Default::default(),
                    rules,
                    labels,
                )))
            }
            (Self::Parse(dst), DataValue::String(s)) => text::parse_value(&s, dst).ok(),
            (Self::Format(src), value) if value.get_type() == *src => {
                Some(DataValue::String(text::format_value(&value)))
            }
            (Self::StringToBool, DataValue::String(s)) => {
                parse_switch_state(&s).map(DataValue::Bool)
            }
            (Self::NumberToBool, DataValue::Number(n)) if n.to_f64().is_finite() => {
                Some(DataValue::Bool(n.to_f64() != 0.0))
            }
            (Self::BoolToNumber, DataValue::Bool(b)) => {
                Some(DataValue::Number(NumberValue::Int(i64::from(b))))
            }
            (Self::StringToEnum(values), DataValue::String(s)) => {
                values.contains(&s).then_some(DataValue::String(s))
            }
            (
                Self::List {
                    element_type,
                    conversion,
                },
                DataValue::List { elements, .. },
            ) => convert_list(element_type, conversion, elements, rules, labels),
            _ => None,
        }
    }
}

pub(super) fn convert_list(
    element_type: &DataType,
    conversion: &Conversion,
    elements: Vec<DataValue>,
    rules: &crate::DisplayRules,
    labels: &dyn crate::quantity::QuantityLabels,
) -> Option<DataValue> {
    Some(DataValue::List {
        element_type: element_type.clone(),
        elements: elements
            .into_iter()
            .map(|value| conversion.apply_with_display(value, rules, labels))
            .collect::<Option<Vec<_>>>()?,
    })
}

pub(super) fn scalar_conversion(src: &DataType, dst: &DataType) -> Option<Conversion> {
    use DataType::*;
    Some(match (src, dst) {
        (String, Number | Time | DateTime | Duration | Color) => Conversion::Parse(dst.clone()),
        (Number | Bool | Time | DateTime | Duration | Color, String) => {
            Conversion::Format(src.clone())
        }
        (String, Bool) => Conversion::StringToBool,
        (Number, Bool) => Conversion::NumberToBool,
        (Bool, Number) => Conversion::BoolToNumber,
        _ => return None,
    })
}

pub(super) fn parse_switch_state(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "open" | "home" | "playing" | "locked" | "detected"
        | "active" | "occupied" | "connected" => Some(true),
        "off" | "false" | "no" | "0" | "closed" | "not_home" | "paused" | "unlocked" | "clear"
        | "inactive" | "unoccupied" | "disconnected" => Some(false),
        _ => None,
    }
}
