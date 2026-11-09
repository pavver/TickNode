//! Value, time and pin labels used by the panels.

use ticknode_shared::*;
use crate::geometry::match_cases;
use crate::i18n::{self, t};

/// Longer values in the inspector are truncated (the full text is shown in the tooltip).
pub(super) const MAX_VALUE_CHARS: usize = 400;

pub fn property_label(name: &str) -> String {
    const LABELS: &[(&str, &str)] = &[
        ("value", "prop-value"),
        ("var_name", "prop-var-name"),
        ("expression", "prop-expression"),
        ("prefix", "prop-prefix"),
        ("label", "prop-label"),
        ("element_type", "prop-element-type"),
        ("field_type", "prop-field-type"),
        ("fields_list", "prop-fields-list"),
        ("interval_ticks", "prop-interval-ticks"),
        ("all_matches", "prop-all-matches"),
        ("cases", "prop-cases"),
    ];
    LABELS.iter().find(|(value, _)| *value == name)
        .map_or_else(|| name.to_string(), |(_, key)| t!(*key))
}

/// Formats a value for display in the inspector: strings are quoted to distinguish `"5"` from `5`.
pub fn display_value(value: &DataValue) -> String {
    match value {
        DataValue::String(s) => format!("\"{s}\""),
        DataValue::Number(NumberValue::Int(i)) => i.to_string(),
        DataValue::Number(NumberValue::Float(f)) => f.to_string(),
        DataValue::Bool(b) => b.to_string(),
        DataValue::Time(t) => t.format("%H:%M:%S").to_string(),
        DataValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        DataValue::Duration(d) => text::format_duration(*d),
        DataValue::Ticks(_) => text::format_value(value),
        DataValue::Color(c) => format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => {
            format!("[{}]", elements.iter().map(display_value).collect::<Vec<_>>().join(", "))
        }
        DataValue::Object { fields, .. } => {
            let fields: Vec<_> = fields.iter().map(|(k, f)| format!("{k}: {}", display_value(&f.value))).collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

pub(super) fn truncate(text: String, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

pub fn relative_time(ms: f64) -> String {
    let secs = (ms.max(0.0) / 1000.0).floor() as u64;
    match secs {
        0 => t!("time-just-now"),
        s if s < 60 => t!("time-seconds-ago", count = s),
        s if s < 3600 => t!("time-minutes-ago", count = s / 60),
        s if s < 86_400 => t!("time-hours-ago", count = s / 3600),
        s => t!("time-days-ago", count = s / 86_400),
    }
}

/// Localized pin label for a node instance on the canvas (including Match case conditions).
pub(super) fn instance_pin_label(instance: &NodeInstance, pin: PinId, dir: PinDirection, fallback: &str) -> String {
    if instance.definition.type_name == "match"
        && dir == PinDirection::Output
        && pin != PinId(0)
        && let Some(case) = match_cases(&instance.model.properties).into_iter().find(|c| c.pin == pin.0)
    {
        return i18n::format_match_case(&case);
    }
    i18n::node_pin_label(&instance.definition.type_name, fallback)
}

#[cfg(test)]
mod tests;
