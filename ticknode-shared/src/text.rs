//! Text representation of values that can be entered from the keyboard (inline node fields, Match conditions):
//! number, string, boolean, time, date-time, duration, color.
//!
//! - time — `HH:MM` or `HH:MM:SS`;
//! - date-time — `YYYY-MM-DD HH:MM[:SS]` (UTC) or RFC 3339 with offset;
//! - duration — seconds (`90`, `1.5`) or with units: `1h 30m`, `45s`, `500ms`, or ticks: `10t` (including localized unit suffixes);
//! - color — `#rrggbb` or `#rrggbbaa`.

use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, NaiveTime, Utc};

use crate::{Color, DataType, DataValue, NumberValue, TypeRegistry};

mod duration;
pub use duration::{format_duration, parse_duration, parse_duration_value};

pub fn parse_number(text: &str) -> Option<NumberValue> {
    if let Ok(n) = text.trim().parse::<i64>() {
        return Some(NumberValue::Int(n));
    }
    let trimmed = text.trim();
    let f: f64 = trimmed.replace(',', ".").parse().ok()?;
    // Out-of-range integer literals must remain floats; normalization could clamp a rounded boundary.
    let digits = trimmed.strip_prefix(['-', '+']).unwrap_or(trimmed);
    let out_of_range_integer = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit());
    f.is_finite().then(|| {
        if out_of_range_integer {
            NumberValue::Float(f)
        } else {
            NumberValue::Float(f).normalize()
        }
    })
}

pub fn parse_time(text: &str) -> Option<NaiveTime> {
    let t = text.trim();
    NaiveTime::parse_from_str(t, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M"))
        .ok()
}

pub fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    let t = text.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Some(dt.with_timezone(&Utc));
    }
    [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ]
    .iter()
    .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
    .or_else(|| {
        chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
    })
    .map(|n| n.and_utc())
}

pub fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.is_ascii() || !(hex.len() == 6 || hex.len() == 8) {
        return None;
    }
    color_from_hex(hex)
}

fn color_from_hex(hex: &str) -> Option<Color> {
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color {
        r: byte(0)?,
        g: byte(2)?,
        b: byte(4)?,
        a: if hex.len() == 8 { byte(6)? } else { 255 },
    })
}

/// Text representation of a value for an input field (unquoted for strings). `parse_value` parses it back.
pub fn format_value(value: &DataValue) -> String {
    match value {
        DataValue::List { elements, .. } => format!(
            "[{}]",
            elements
                .iter()
                .map(format_value)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        DataValue::Object { fields, .. } => {
            let fields: Vec<_> = fields
                .iter()
                .map(|(k, f)| format!("{k}: {}", format_value(&f.value)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
        _ => format_scalar(value).expect("a non-collection is a scalar"),
    }
}

fn format_scalar(value: &DataValue) -> Option<String> {
    Some(match value {
        DataValue::String(s) => s.clone(),
        DataValue::Number(n) => format_number(*n),
        DataValue::Bool(b) => b.to_string(),
        DataValue::Time(t) => format_time(*t),
        DataValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        DataValue::Duration(d) => format_duration(*d),
        DataValue::Ticks(n) => format!("{n}t"),
        DataValue::Color(c) => format_color(*c),
        _ => return None,
    })
}

fn format_number(n: NumberValue) -> String {
    match n {
        NumberValue::Int(i) => i.to_string(),
        NumberValue::Float(f) => f.to_string(),
    }
}

fn format_time(time: NaiveTime) -> String {
    time.format(if time.format("%S").to_string() == "00" {
        "%H:%M"
    } else {
        "%H:%M:%S"
    })
    .to_string()
}

fn format_color(color: Color) -> String {
    match color {
        Color { r, g, b, a: 255 } => format!("#{r:02x}{g:02x}{b:02x}"),
        Color { r, g, b, a } => format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
    }
}

/// Value of unknown type: number, `true`/`false`, otherwise string (quotes allow writing
/// the string `"5"`).
pub fn parse_any(text: &str) -> DataValue {
    let t = text.trim();
    if let Some(n) = parse_number(t) {
        return DataValue::Number(n);
    }
    match t {
        "true" => return DataValue::Bool(true),
        "false" => return DataValue::Bool(false),
        _ => {}
    }
    match t.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        Some(inner) => DataValue::String(inner.to_string()),
        None => DataValue::String(text.to_string()),
    }
}

/// Whether a value of this type can be entered as text.
pub fn is_text_type(t: &DataType) -> bool {
    matches!(
        t,
        DataType::Number
            | DataType::String
            | DataType::Bool
            | DataType::Time
            | DataType::DateTime
            | DataType::Duration
            | DataType::Color
            | DataType::Any
    )
}

/// Registry-aware text types also include enums and physical quantities.
pub fn is_text_type_in(types: &TypeRegistry, t: &DataType) -> bool {
    is_text_type(t)
        || types
            .definition_of(t)
            .is_some_and(|d| d.is_enum() || d.is_identifier() || d.quantity().is_some())
}

/// Placeholder hint for an input field of this type.
pub fn type_hint(t: &DataType) -> &'static str {
    match t {
        DataType::Time => "HH:MM",
        DataType::DateTime => "YYYY-MM-DD HH:MM",
        DataType::Duration => "10t / 1h 30m / 45s",
        DataType::Color => "#rrggbb",
        _ => "—",
    }
}

/// Default value for a new field of type `t` (`Any` defaults to number 0).
pub fn default_of(t: &DataType) -> DataValue {
    match t {
        DataType::String => DataValue::String(String::new()),
        DataType::Bool => DataValue::Bool(true),
        DataType::Time => DataValue::Time(NaiveTime::MIN),
        DataType::DateTime => DataValue::DateTime(DateTime::<Utc>::UNIX_EPOCH),
        DataType::Duration => DataValue::Duration(Duration::from_secs(1)),
        DataType::Color => DataValue::Color(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }),
        DataType::List(inner) => DataValue::List {
            element_type: (**inner).clone(),
            elements: Vec::new(),
        },
        DataType::Named(name) if crate::quantity::kind(name).is_some() => DataValue::from(0),
        DataType::Object | DataType::Named(_) => DataValue::Object {
            fields: Default::default(),
            type_name: None,
        },
        DataType::Number | DataType::Any => DataValue::Number(NumberValue::Int(0)),
    }
}

/// Parses text as a value of type `t`.
pub fn parse_value(text: &str, t: &DataType) -> Result<DataValue, String> {
    match t {
        DataType::String => Ok(DataValue::String(text.to_string())),
        DataType::Any => Ok(parse_any(text)),
        _ => parse_typed_text(text, t),
    }
}

type TextParser = fn(&str) -> Option<DataValue>;

fn parse_typed_text(text: &str, t: &DataType) -> Result<DataValue, String> {
    const PARSERS: &[(DataType, TextParser, &str)] = &[
        (
            DataType::Number,
            |s| parse_number(s).map(DataValue::Number),
            "a number",
        ),
        (DataType::Bool, parse_bool, "true/false"),
        (
            DataType::Time,
            |s| parse_time(s).map(DataValue::Time),
            "a time (HH:MM or HH:MM:SS)",
        ),
        (
            DataType::DateTime,
            |s| parse_datetime(s).map(DataValue::DateTime),
            "a date-time (YYYY-MM-DD HH:MM)",
        ),
        (
            DataType::Duration,
            parse_duration_value,
            "a duration (10t, 90, 1h 30m, 500ms)",
        ),
        (
            DataType::Color,
            |s| parse_color(s).map(DataValue::Color),
            "a color (#rrggbb)",
        ),
    ];
    let Some((_, parser, description)) = PARSERS.iter().find(|(ty, _, _)| ty == t) else {
        return Err(format!(
            "a value of type {} cannot be entered as text",
            crate::data_type_name(t)
        ));
    };
    parser(text).ok_or_else(|| format!("\"{}\" is not {description}", text.trim()))
}

fn parse_bool(text: &str) -> Option<DataValue> {
    match text.trim() {
        "true" | "так" | "1" => Some(DataValue::Bool(true)),
        "false" | "ні" | "0" => Some(DataValue::Bool(false)),
        _ => None,
    }
}

/// Like `parse_value`, but custom-type aware: an enum value is a string from the enumeration
/// (matched by value or label), while a struct cannot be entered as text.
pub fn parse_value_in(types: &TypeRegistry, text: &str, t: &DataType) -> Result<DataValue, String> {
    match types.definition_of(t) {
        Some(def) if def.is_identifier() => text
            .trim()
            .parse::<i64>()
            .map(DataValue::from)
            .map_err(|_| format!("\"{}\" is not a signed 64-bit integer", text.trim())),
        Some(def) if def.quantity().is_some() => {
            crate::quantity::parse_quantity(text, def.quantity().unwrap())
                .map(DataValue::from)
                .ok_or_else(|| format!("\"{}\" is not a value of {}", text.trim(), def.title))
        }
        Some(def) if def.is_enum() => {
            let wanted = text.trim();
            def.values()
                .iter()
                .find(|v| v.value == wanted)
                .or_else(|| def.values().iter().find(|v| v.title == wanted))
                .map(|v| DataValue::String(v.value.clone()))
                .ok_or_else(|| {
                    let all: Vec<_> = def.values().iter().map(|v| v.value.as_str()).collect();
                    format!(
                        "\"{wanted}\" is not a value of {} (allowed: {})",
                        def.title,
                        all.join(", ")
                    )
                })
        }
        Some(def) => Err(format!(
            "a value of type {} cannot be entered as text",
            def.title
        )),
        None => parse_value(text, t),
    }
}

/// Coerces `value` to type `like` where unambiguous: string → time / date / color / number,
/// number → duration (seconds). Otherwise returns `value` as-is.
pub fn coerce_to(value: DataValue, like: &DataType) -> DataValue {
    match (&value, like) {
        (
            DataValue::String(s),
            DataType::Number
            | DataType::Bool
            | DataType::Time
            | DataType::DateTime
            | DataType::Duration
            | DataType::Color,
        ) => parse_value(s, like).unwrap_or(value),
        // Infinity, negative, or out-of-range numbers remain numbers — comparison will yield a type mismatch
        (DataValue::Number(n), DataType::Duration) => match Duration::try_from_secs_f64(n.to_f64())
        {
            Ok(d) => DataValue::Duration(d),
            Err(_) => value,
        },
        _ => value,
    }
}

/// Registry-aware constant coercion, including strings with quantity units.
pub fn coerce_to_in(types: &TypeRegistry, value: DataValue, like: &DataType) -> DataValue {
    if let DataValue::String(s) = &value
        && types.definition_of(like).is_some()
    {
        return parse_value_in(types, s, like).unwrap_or(value);
    }
    coerce_to(value, like)
}

#[cfg(test)]
mod tests;
