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

pub fn parse_number(text: &str) -> Option<NumberValue> {
    let f: f64 = text.trim().replace(',', ".").parse().ok()?;
    f.is_finite().then(|| NumberValue::Float(f).normalize())
}

pub fn parse_time(text: &str) -> Option<NaiveTime> {
    let t = text.trim();
    NaiveTime::parse_from_str(t, "%H:%M:%S").or_else(|_| NaiveTime::parse_from_str(t, "%H:%M")).ok()
}

pub fn parse_datetime(text: &str) -> Option<DateTime<Utc>> {
    let t = text.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(t) {
        return Some(dt.with_timezone(&Utc));
    }
    ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|f| NaiveDateTime::parse_from_str(t, f).ok())
        .or_else(|| chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d").ok().and_then(|d| d.and_hms_opt(0, 0, 0)))
        .map(|n| n.and_utc())
}

/// `90` / `1.5` — seconds; otherwise a sequence of "number unit" pairs: `1h 30m`, `45s`, `500ms`.
pub fn parse_duration(text: &str) -> Option<Duration> {
    let t = text.trim().replace(',', ".");
    if let Ok(secs) = t.parse::<f64>() {
        return Duration::try_from_secs_f64(secs).ok();
    }
    let chars: Vec<char> = t.chars().collect();
    let mut i = 0;
    let mut total = 0.0;
    let mut parts = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        total += duration_part(&chars, &mut i)?;
        parts += 1;
    }
    if parts == 0 {
        return None;
    }
    Duration::try_from_secs_f64(total).ok()
}

/// Parses a tick count with an English or Ukrainian suffix, or a wall-clock duration.
pub fn parse_duration_value(text: &str) -> Option<DataValue> {
    let lower = text.trim().to_lowercase();
    for suffix in ["ticks", "tick", "t", "тактів", "такти", "такт"] {
        if let Some(number) = lower.strip_suffix(suffix) {
            let number = number.trim();
            return (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
                .then(|| number.parse().ok()).flatten().map(DataValue::Ticks);
        }
    }
    parse_duration(text).map(DataValue::Duration)
}

fn duration_part(chars: &[char], position: &mut usize) -> Option<f64> {
    let mut i = *position;
    let start = i;
    while chars.get(i).is_some_and(|ch| ch.is_ascii_digit() || *ch == '.') {
        i += 1;
    }
    let number: f64 = chars[start..i].iter().collect::<String>().parse().ok()?;
    while chars.get(i).is_some_and(|ch| ch.is_whitespace()) {
        i += 1;
    }
    let unit_start = i;
    while chars.get(i).is_some_and(|ch| ch.is_alphabetic()) {
        i += 1;
    }
    let unit: String = chars[unit_start..i].iter().collect::<String>().to_lowercase();
    let scale = duration_scale(&unit)?;
    *position = i;
    Some(number * scale)
}

fn duration_scale(unit: &str) -> Option<f64> {
    Some(match unit {
        "ms" | "мс" => 0.001,
        "s" | "sec" | "с" | "сек" => 1.0,
        "m" | "min" | "хв" => 60.0,
        "h" | "год" => 3600.0,
        "d" | "дн" | "д" => 86_400.0,
        _ => return None,
    })
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
    Some(Color { r: byte(0)?, g: byte(2)?, b: byte(4)?, a: if hex.len() == 8 { byte(6)? } else { 255 } })
}

pub fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms == 0 {
        return "0s".into();
    }
    if ms < 1000 {
        return format!("{ms}ms");
    }
    let mut secs = d.as_secs();
    let mut parts = Vec::new();
    for (unit, size) in [("d", 86_400), ("h", 3600), ("m", 60)] {
        if secs >= size {
            parts.push(format!("{}{unit}", secs / size));
            secs %= size;
        }
    }
    // Subsecond fraction goes into the last part: `1m 1.5s`, not `1m 1s`
    let nanos = d.subsec_nanos();
    if nanos != 0 {
        parts.push(format!("{}s", secs as f64 + f64::from(nanos) / 1e9));
    } else if secs > 0 {
        parts.push(format!("{secs}s"));
    }
    parts.join(" ")
}

/// Text representation of a value for an input field (unquoted for strings). `parse_value` parses it back.
pub fn format_value(value: &DataValue) -> String {
    match value {
        DataValue::List { elements, .. } => format!("[{}]", elements.iter().map(format_value).collect::<Vec<_>>().join(", ")),
        DataValue::Object { fields, .. } => {
            let fields: Vec<_> = fields.iter().map(|(k, f)| format!("{k}: {}", format_value(&f.value))).collect();
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
    match n { NumberValue::Int(i) => i.to_string(), NumberValue::Float(f) => f.to_string() }
}

fn format_time(time: NaiveTime) -> String {
    time.format(if time.format("%S").to_string() == "00" { "%H:%M" } else { "%H:%M:%S" }).to_string()
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
        DataType::Number | DataType::String | DataType::Bool | DataType::Time | DataType::DateTime
            | DataType::Duration | DataType::Color | DataType::Any
    )
}

/// Registry-aware text types also include enums and physical quantities.
pub fn is_text_type_in(types: &TypeRegistry, t: &DataType) -> bool {
    is_text_type(t) || types.definition_of(t).is_some_and(|d| d.is_enum() || d.quantity().is_some())
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
        DataType::Color => DataValue::Color(Color { r: 255, g: 255, b: 255, a: 255 }),
        DataType::List(inner) => DataValue::List { element_type: (**inner).clone(), elements: Vec::new() },
        DataType::Named(name) if crate::quantity::kind(name).is_some() => DataValue::from(0),
        DataType::Object | DataType::Named(_) => DataValue::Object { fields: Default::default(), type_name: None },
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
        (DataType::Number, |s| parse_number(s).map(DataValue::Number), "a number"),
        (DataType::Bool, parse_bool, "true/false"),
        (DataType::Time, |s| parse_time(s).map(DataValue::Time), "a time (HH:MM or HH:MM:SS)"),
        (DataType::DateTime, |s| parse_datetime(s).map(DataValue::DateTime), "a date-time (YYYY-MM-DD HH:MM)"),
        (DataType::Duration, parse_duration_value, "a duration (10t, 90, 1h 30m, 500ms)"),
        (DataType::Color, |s| parse_color(s).map(DataValue::Color), "a color (#rrggbb)"),
    ];
    let Some((_, parser, description)) = PARSERS.iter().find(|(ty, _, _)| ty == t) else {
        return Err(format!("a value of type {} cannot be entered as text", crate::data_type_name(t)));
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
        Some(def) if def.quantity().is_some() => crate::quantity::parse_quantity(text, def.quantity().unwrap())
            .map(DataValue::from).ok_or_else(|| format!("\"{}\" is not a value of {}", text.trim(), def.title)),
        Some(def) if def.is_enum() => {
            let wanted = text.trim();
            def.values().iter()
                .find(|v| v.value == wanted)
                .or_else(|| def.values().iter().find(|v| v.title == wanted))
                .map(|v| DataValue::String(v.value.clone()))
                .ok_or_else(|| {
                    let all: Vec<_> = def.values().iter().map(|v| v.value.as_str()).collect();
                    format!("\"{wanted}\" is not a value of {} (allowed: {})", def.title, all.join(", "))
                })
        }
        Some(def) => Err(format!("a value of type {} cannot be entered as text", def.title)),
        None => parse_value(text, t),
    }
}

/// Coerces `value` to type `like` where unambiguous: string → time / date / color / number,
/// number → duration (seconds). Otherwise returns `value` as-is.
pub fn coerce_to(value: DataValue, like: &DataType) -> DataValue {
    match (&value, like) {
        (DataValue::String(s), DataType::Number | DataType::Bool | DataType::Time | DataType::DateTime | DataType::Duration | DataType::Color) => {
            parse_value(s, like).unwrap_or(value)
        }
        // Infinity, negative, or out-of-range numbers remain numbers — comparison will yield a type mismatch
        (DataValue::Number(n), DataType::Duration) => match Duration::try_from_secs_f64(n.to_f64()) {
            Ok(d) => DataValue::Duration(d),
            Err(_) => value,
        },
        _ => value,
    }
}

/// Registry-aware constant coercion, including strings with quantity units.
pub fn coerce_to_in(types: &TypeRegistry, value: DataValue, like: &DataType) -> DataValue {
    if let DataValue::String(s) = &value && types.definition_of(like).is_some() {
        return parse_value_in(types, s, like).unwrap_or(value);
    }
    coerce_to(value, like)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for (text, t) in [
            ("42", DataType::Number),
            ("2.5", DataType::Number),
            ("hello", DataType::String),
            ("true", DataType::Bool),
            ("07:30", DataType::Time),
            ("07:30:15", DataType::Time),
            ("2026-10-29 18:05:00", DataType::DateTime),
            ("1h 30m", DataType::Duration),
            ("500ms", DataType::Duration),
            ("1m 1.5s", DataType::Duration),
            ("1h 0.25s", DataType::Duration),
            ("#ff8800", DataType::Color),
            ("#ff880080", DataType::Color),
        ] {
            let v = parse_value(text, &t).unwrap();
            assert_eq!(v.get_type(), t);
            assert_eq!(format_value(&v), text, "{t:?}");
        }
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("90"), Some(Duration::from_secs(90)));
        assert_eq!(parse_duration("2хв 10с"), Some(Duration::from_secs(130)));
        assert_eq!(parse_duration("1.5h"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("5 parsecs"), None);
        assert_eq!(format_duration(Duration::from_millis(1500)), "1.5s");
    }

    #[test]
    fn errors_and_coercion() {
        assert!(parse_value("25:00", &DataType::Time).is_err());
        assert!(parse_value("red", &DataType::Color).is_err());
        assert!(parse_value("x", &DataType::Number).is_err());
        assert_eq!(coerce_to(DataValue::String("07:30".into()), &DataType::Time), DataValue::Time(parse_time("07:30").unwrap()));
        assert_eq!(coerce_to(DataValue::Number(NumberValue::Int(60)), &DataType::Duration), DataValue::Duration(Duration::from_secs(60)));
        assert_eq!(coerce_to(DataValue::String("abc".into()), &DataType::Time), DataValue::String("abc".into()));
        // Infinity and out-of-range numbers are not converted (and do not panic)
        for n in [f64::INFINITY, 1e30, -1.0] {
            let v = DataValue::Number(NumberValue::Float(n));
            assert_eq!(coerce_to(v.clone(), &DataType::Duration), v);
        }
        assert_eq!(parse_duration("1e30"), None);
        assert_eq!(parse_duration("99999999999999999999999d"), None);
        assert_eq!(parse_any("\"5\""), DataValue::String("5".into()));
        assert_eq!(parse_datetime("2026-10-29T10:00:00+02:00").unwrap().format("%H").to_string(), "08");
    }
}
