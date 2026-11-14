//! Duration formatting filter (`duration`) for MiniJinja templates.

use crate::plural;
use crate::quantity::QuantityLabels;
use chrono::TimeDelta;
use minijinja::{Environment, ErrorKind, Value};
use std::fmt::Write;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub fn parse_duration_input(value: &Value) -> Result<Option<TimeDelta>, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(None);
    }
    if value.kind() == minijinja::value::ValueKind::Number {
        if let Some(i) = value.as_i64() {
            return crate::delta_from_secs_f64(i as f64)
                .map(Some)
                .ok_or_else(|| {
                    minijinja::Error::new(
                        ErrorKind::InvalidOperation,
                        "duration number out of range",
                    )
                });
        }
        if let Ok(f) = f64::try_from(value.clone()) {
            return crate::delta_from_secs_f64(f).map(Some).ok_or_else(|| {
                minijinja::Error::new(ErrorKind::InvalidOperation, "duration number out of range")
            });
        }
    }
    if let Some(s) = value.as_str() {
        if matches!(
            crate::text::parse_duration_value(s),
            Some(crate::DataValue::Ticks(_))
        ) {
            return Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                "duration needs a wall-clock duration",
            ));
        }
        if let Some(d) = crate::text::parse_duration(s) {
            return Ok(Some(d));
        }
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            "duration expects a duration or seconds",
        ));
    }
    Err(minijinja::Error::new(
        ErrorKind::InvalidOperation,
        "duration expects a duration or seconds",
    ))
}

pub fn format_duration_short(d: TimeDelta, labels: &dyn QuantityLabels) -> String {
    crate::text::format_duration(d)
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

pub fn format_duration_long(d: TimeDelta, labels: &dyn QuantityLabels) -> String {
    let is_negative = d < TimeDelta::zero();
    let total_secs = d.num_seconds().abs();
    let days = total_secs / 86_400;
    let rem = total_secs % 86_400;
    let hours = rem / 3600;
    let rem = rem % 3600;
    let minutes = rem / 60;
    let seconds = rem % 60;

    let mut parts = Vec::new();

    let mut push_unit = |unit: &str, n: i64, unit_sing: &str, unit_plur: &str| {
        if n > 0 {
            let cat = plural::category(&labels.locale(), n as f64);
            let key = format!("text-duration-long-{unit}-{}", cat.as_str());
            let n_str = n.to_string();
            let text = labels
                .message_args(&key, &[("n", n_str.clone())])
                .or_else(|| {
                    let fb_key = format!("text-duration-long-{unit}-other");
                    labels.message_args(&fb_key, &[("n", n_str.clone())])
                })
                .unwrap_or_else(|| {
                    if n == 1 {
                        format!("1 {unit_sing}")
                    } else {
                        format!("{n} {unit_plur}")
                    }
                });
            parts.push(text);
        }
    };

    push_unit("days", days, "day", "days");
    push_unit("hours", hours, "hour", "hours");
    push_unit("minutes", minutes, "minute", "minutes");
    push_unit("seconds", seconds, "second", "seconds");

    if parts.is_empty() {
        let cat = plural::category(&labels.locale(), 0.0);
        let key = format!("text-duration-long-seconds-{}", cat.as_str());
        let text = labels
            .message_args(&key, &[("n", "0".to_string())])
            .or_else(|| {
                labels.message_args(
                    "text-duration-long-seconds-other",
                    &[("n", "0".to_string())],
                )
            })
            .unwrap_or_else(|| "0 seconds".to_string());
        parts.push(text);
    }

    let mut res = parts.join(" ");
    if is_negative && total_secs > 0 {
        res.insert(0, '-');
    }
    res
}

enum DurationItem {
    Days,
    Hours2,
    Hours1,
    Minutes2,
    Minutes1,
    Seconds2,
    Seconds1,
    Millis,
    Literal(String),
}

fn parse_duration_pattern(pattern: &str) -> Vec<DurationItem> {
    let bytes = pattern.as_bytes();
    let mut items = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b']' {
                i += 1;
            }
            let lit = &pattern[start..i];
            items.push(DurationItem::Literal(lit.to_string()));
            if i < bytes.len() && bytes[i] == b']' {
                i += 1;
            }
            continue;
        }
        if bytes[i..].starts_with(b"HH") {
            items.push(DurationItem::Hours2);
            i += 2;
        } else if bytes[i..].starts_with(b"H") {
            items.push(DurationItem::Hours1);
            i += 1;
        } else if bytes[i..].starts_with(b"mm") {
            items.push(DurationItem::Minutes2);
            i += 2;
        } else if bytes[i..].starts_with(b"m") {
            items.push(DurationItem::Minutes1);
            i += 1;
        } else if bytes[i..].starts_with(b"ss") {
            items.push(DurationItem::Seconds2);
            i += 2;
        } else if bytes[i..].starts_with(b"s") {
            items.push(DurationItem::Seconds1);
            i += 1;
        } else if bytes[i..].starts_with(b"SSS") {
            items.push(DurationItem::Millis);
            i += 3;
        } else if bytes[i..].starts_with(b"d") {
            items.push(DurationItem::Days);
            i += 1;
        } else if let Some(ch) = pattern[i..].chars().next() {
            let mut s = String::new();
            s.push(ch);
            items.push(DurationItem::Literal(s));
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    items
}

pub fn format_duration_pattern(pattern: &str, d: TimeDelta) -> String {
    let items = parse_duration_pattern(pattern);
    let has_d = items.iter().any(|item| matches!(item, DurationItem::Days));
    let has_h = items
        .iter()
        .any(|item| matches!(item, DurationItem::Hours1 | DurationItem::Hours2));

    let is_negative = d < TimeDelta::zero();
    let total_secs = d.num_seconds().abs();
    let millis = (d.subsec_nanos().abs() / 1_000_000) as u32;

    let days = total_secs / 86_400;
    let hours = if has_d {
        (total_secs % 86_400) / 3600
    } else {
        total_secs / 3600
    };
    let minutes = if has_d || has_h {
        (total_secs % 3600) / 60
    } else {
        total_secs / 60
    };
    let seconds = total_secs % 60;

    let mut out = String::new();
    if is_negative {
        out.push('-');
    }
    for item in items {
        match item {
            DurationItem::Days => {
                let _ = write!(out, "{days}");
            }
            DurationItem::Hours2 => {
                let _ = write!(out, "{hours:02}");
            }
            DurationItem::Hours1 => {
                let _ = write!(out, "{hours}");
            }
            DurationItem::Minutes2 => {
                let _ = write!(out, "{minutes:02}");
            }
            DurationItem::Minutes1 => {
                let _ = write!(out, "{minutes}");
            }
            DurationItem::Seconds2 => {
                let _ = write!(out, "{seconds:02}");
            }
            DurationItem::Seconds1 => {
                let _ = write!(out, "{seconds}");
            }
            DurationItem::Millis => {
                let _ = write!(out, "{millis:03}");
            }
            DurationItem::Literal(lit) => {
                out.push_str(&lit);
            }
        }
    }
    out
}

pub fn format_duration_preset(
    preset_or_pattern: &str,
    d: TimeDelta,
    labels: &dyn QuantityLabels,
) -> String {
    match preset_or_pattern {
        "short" => format_duration_short(d, labels),
        "long" => format_duration_long(d, labels),
        pat => format_duration_pattern(pat, d),
    }
}

pub fn duration_filter(
    labels: &dyn QuantityLabels,
    value: Value,
    pattern: Option<String>,
) -> Result<Value, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(Value::UNDEFINED);
    }
    let Some(d) = parse_duration_input(&value)? else {
        return Ok(Value::UNDEFINED);
    };

    let result = match pattern.as_deref() {
        None | Some("short") => format_duration_short(d, labels),
        Some("long") => format_duration_long(d, labels),
        Some(pat) => format_duration_pattern(pat, d),
    };
    Ok(Value::from(result))
}

pub fn example(pattern_or_preset: &str, labels: Option<&dyn QuantityLabels>) -> String {
    let sample = TimeDelta::days(1)
        + TimeDelta::hours(2)
        + TimeDelta::minutes(5)
        + TimeDelta::seconds(7)
        + TimeDelta::milliseconds(250);
    let canonical = crate::quantity::CanonicalLabels;
    let lbl = labels.unwrap_or(&canonical);
    format_duration_preset(pattern_or_preset, sample, lbl)
}

pub(super) fn register(
    env: &mut Environment<'static>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) {
    let duration_labels = Arc::clone(&labels);
    env.add_filter("duration", move |value: Value, pattern: Option<String>| {
        duration_filter(&*duration_labels, value, pattern)
    });
}
