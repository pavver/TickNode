//! Translated UI messages, value hints and Match condition presentation.

use fluent_bundle::{FluentArgs, FluentValue};
use ticknode_shared::condition::{CompareOp, Join, MatchCase, Operand};
use ticknode_shared::*;
use super::{lookup, slug, t, try_translate, type_def_title};

/// Text of a message for the user: the translation of its key (searched in the editor's own and the
/// server-provided texts, arguments substituted), otherwise the English fallback.
pub fn ui_text(message: &UiMessage) -> String {
    if !message.key.is_empty() {
        let mut args = FluentArgs::new();
        // A setting error names the setting by its translated title (`setting-<plugin>-<key>-title`)
        let setting_title = message.args.iter().find(|(n, _)| n == "plugin").zip(message.args.iter().find(|(n, _)| n == "key"))
            .and_then(|((_, plugin), (_, key))| lookup(&format!("setting-{}-{}-title", slug(plugin), slug(key))));
        for (name, value) in &message.args {
            let value = match (&setting_title, name.as_str()) {
                (Some(title), "setting") => title.clone(),
                _ => value.clone(),
            };
            args.set(name.clone(), FluentValue::from(value));
        }
        if let Some(text) = try_translate(&message.key, Some(&args)) {
            return text;
        }
    }
    message.fallback.clone()
}

// --- Data types, field hints, and value parsing ---

pub fn ticks_text(count: u64) -> String {
    t!("duration-ticks", count = count)
}

pub fn type_name_label(name: &str) -> String {
    const LABELS: &[(&str, &str)] = &[
        ("exec", "type-exec"),
        ("number", "type-number"),
        ("string", "type-string"),
        ("bool", "type-bool"),
        ("time", "type-time"),
        ("datetime", "type-datetime"),
        ("duration", "type-duration"),
        ("color", "type-color"),
        ("object", "type-object"),
        ("list", "type-list"),
        ("any", "type-any"),
        ("auto", "type-auto"),
    ];
    LABELS.iter().find(|(value, _)| *value == name)
        .map_or_else(|| name.to_string(), |(_, key)| t!(*key))
}

pub fn data_type_title(types: &TypeRegistry, t: &DataType) -> String {
    match t {
        DataType::Named(name) => types.get(name).map_or_else(|| t!("type-unavailable"), type_def_title),
        DataType::List(inner) => t!("type-list-of", inner = data_type_title(types, inner)),
        other => type_name_label(&data_type_name(other)),
    }
}

pub fn type_hint(t: &DataType) -> String {
    match t {
        DataType::Time => t!("hint-time"),
        DataType::DateTime => t!("hint-datetime"),
        DataType::Duration => t!("hint-duration"),
        DataType::Color => t!("hint-color"),
        _ => t!("hint-default"),
    }
}

/// Date-times are entered and shown in the browser time zone; values stay UTC instants.
pub fn local_datetime_text(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Text for an editable field: like `text::format_value`, but date-times in the browser time zone.
pub fn value_edit_text(value: &DataValue) -> String {
    match value {
        DataValue::DateTime(dt) => local_datetime_text(dt),
        other => text::format_value(other),
    }
}

/// A date-time without an explicit offset is local browser time; RFC 3339 text keeps its offset.
pub fn parse_local_datetime(input: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{Local, NaiveDateTime, TimeZone, Utc};
    let t = input.trim();
    if chrono::DateTime::parse_from_rfc3339(t).is_ok() {
        return text::parse_datetime(t);
    }
    let naive = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)))?;
    Local.from_local_datetime(&naive).earliest().map(|dt| dt.with_timezone(&Utc))
}

pub fn parse_value_localized(input: &str, t: &DataType) -> Result<DataValue, String> {
    let trimmed = input.trim();
    let parsed = match t {
        DataType::DateTime => parse_local_datetime(input).map(DataValue::DateTime).ok_or(()),
        _ => text::parse_value(input, t).map_err(|_| ()),
    };
    parsed.map_err(|_| match t {
        DataType::Number => t!("parse-error-number", value = trimmed),
        DataType::Bool => t!("parse-error-bool", value = trimmed),
        DataType::Time => t!("parse-error-time", value = trimmed),
        DataType::DateTime => t!("parse-error-datetime", value = trimmed),
        DataType::Duration => t!("parse-error-duration", value = trimmed),
        DataType::Color => t!("parse-error-color", value = trimmed),
        _ => t!("parse-error-not-text", type = data_type_name(t)),
    })
}

// --- Match condition operators and formatting ---

pub fn op_symbol(op: CompareOp) -> String {
    match op {
        CompareOp::Eq => t!("match-op-eq"),
        CompareOp::Ne => t!("match-op-ne"),
        CompareOp::Lt => t!("match-op-lt"),
        CompareOp::Le => t!("match-op-le"),
        CompareOp::Gt => t!("match-op-gt"),
        CompareOp::Ge => t!("match-op-ge"),
        CompareOp::Contains => t!("match-op-contains"),
        CompareOp::NotContains => t!("match-op-not-contains"),
        CompareOp::StartsWith => t!("match-op-starts-with"),
        CompareOp::EndsWith => t!("match-op-ends-with"),
        CompareOp::IsEmpty => t!("match-op-is-empty"),
        CompareOp::NotEmpty => t!("match-op-not-empty"),
        CompareOp::IsTrue => t!("match-op-is-true"),
        CompareOp::IsFalse => t!("match-op-is-false"),
        CompareOp::HasField => t!("match-op-has-field"),
        CompareOp::NoField => t!("match-op-no-field"),
        CompareOp::LenEq => t!("match-op-len-eq"),
        CompareOp::LenGt => t!("match-op-len-gt"),
        CompareOp::LenLt => t!("match-op-len-lt"),
        CompareOp::HasValue => t!("match-op-has-value"),
        CompareOp::NoValue => t!("match-op-no-value"),
    }
}

pub fn op_label_for(op: CompareOp, t: &DataType) -> String {
    let time = matches!(t, DataType::Time | DataType::DateTime);
    match op {
        CompareOp::Lt if time => t!("match-op-lt-time"),
        CompareOp::Le if time => t!("match-op-le-time"),
        CompareOp::Gt if time => t!("match-op-gt-time"),
        CompareOp::Ge if time => t!("match-op-ge-time"),
        CompareOp::Lt if *t == DataType::Duration => t!("match-op-lt-duration"),
        CompareOp::Gt if *t == DataType::Duration => t!("match-op-gt-duration"),
        other => op_symbol(other),
    }
}

pub fn join_symbol(join: Join) -> String {
    match join {
        Join::And => t!("match-join-and"),
        Join::Or => t!("match-join-or"),
    }
}

pub fn format_match_case(case: &MatchCase) -> String {
    if case.conditions.is_empty() {
        return t!("match-always");
    }
    case.conditions
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let cond = match &c.right {
                Some(r) => {
                    let right = match r {
                        Operand::Value(DataValue::Ticks(n)) => ticks_text(*n),
                        other => other.to_text(),
                    };
                    format!("{} {} {}", c.left.to_text(), op_symbol(c.op), right)
                }
                None => format!("{} {}", c.left.to_text(), op_symbol(c.op)),
            };
            if i == 0 {
                cond
            } else {
                format!("{} {}", join_symbol(c.join), cond)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
