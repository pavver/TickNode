//! Date and time formatting filters (`date` and `strftime`) for MiniJinja templates.

use crate::quantity::QuantityLabels;
use chrono::{DateTime, Datelike, NaiveTime, Timelike, format::Item, format::StrftimeItems};
use minijinja::{Environment, ErrorKind, Value};
use std::fmt::Write;
use std::sync::Arc;

#[cfg(test)]
mod tests;
mod tokens;
use tokens::*;

pub(crate) enum Moment {
    DateTime(DateTime<chrono::Local>),
    Time(NaiveTime),
}

fn parse_timestamp(f: f64) -> Option<DateTime<chrono::Local>> {
    if !f.is_finite() || f < (i64::MIN as f64) || f > (i64::MAX as f64) {
        return None;
    }
    let mut secs = f.floor() as i64;
    let mut nanos = ((f - f.floor()) * 1_000_000_000.0).round() as u32;
    if nanos >= 1_000_000_000 {
        secs = secs.checked_add(1)?;
        nanos -= 1_000_000_000;
    }
    DateTime::from_timestamp(secs, nanos).map(|dt| dt.with_timezone(&chrono::Local))
}

pub(crate) fn parse_moment(
    value: &Value,
    filter: &str,
) -> Result<Option<Moment>, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(None);
    }
    if let Some(s) = value.as_str() {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return Ok(Some(Moment::DateTime(dt.with_timezone(&chrono::Local))));
        }
        if let Ok(crate::DataValue::DateTime(dt)) =
            crate::text::parse_value(s, &crate::DataType::DateTime)
        {
            return Ok(Some(Moment::DateTime(dt.with_timezone(&chrono::Local))));
        }
        if let Ok(crate::DataValue::Time(t)) = crate::text::parse_value(s, &crate::DataType::Time) {
            return Ok(Some(Moment::Time(t)));
        }
        return Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("{filter} expects a date-time, a time or a Unix timestamp"),
        ));
    }
    if value.kind() == minijinja::value::ValueKind::Number {
        if let Some(i) = value.as_i64() {
            let dt = DateTime::from_timestamp(i, 0)
                .map(|dt| dt.with_timezone(&chrono::Local))
                .ok_or_else(|| {
                    minijinja::Error::new(
                        ErrorKind::InvalidOperation,
                        format!("{filter} expects a valid Unix timestamp"),
                    )
                })?;
            return Ok(Some(Moment::DateTime(dt)));
        }
        if let Ok(f) = f64::try_from(value.clone()) {
            let dt = parse_timestamp(f).ok_or_else(|| {
                minijinja::Error::new(
                    ErrorKind::InvalidOperation,
                    format!("{filter} expects a valid Unix timestamp"),
                )
            })?;
            return Ok(Some(Moment::DateTime(dt)));
        }
    }
    Err(minijinja::Error::new(
        ErrorKind::InvalidOperation,
        format!("{filter} expects a date-time, a time or a Unix timestamp"),
    ))
}

fn format_token(
    out: &mut String,
    token: Token,
    moment: &Moment,
    has_day_token: bool,
    labels: &dyn QuantityLabels,
) {
    if token.is_date_token() {
        let Moment::DateTime(dt) = moment else {
            return;
        };
        match token {
            Token::Year4 => {
                let _ = write!(out, "{:04}", dt.year());
            }
            Token::Year2 => {
                let _ = write!(out, "{:02}", dt.year().rem_euclid(100));
            }
            Token::MonthName => {
                let month = dt.month() as usize;
                let name = if has_day_token {
                    labels
                        .message(&format!("text-month-{month}-format"))
                        .or_else(|| labels.message(&format!("text-month-{month}")))
                        .unwrap_or_else(|| ENGLISH_MONTHS[month - 1].to_string())
                } else {
                    labels
                        .message(&format!("text-month-{month}"))
                        .unwrap_or_else(|| ENGLISH_MONTHS[month - 1].to_string())
                };
                out.push_str(&name);
            }
            Token::MonthShort => {
                let month = dt.month() as usize;
                let name = labels
                    .message(&format!("text-month-{month}-short"))
                    .unwrap_or_else(|| ENGLISH_MONTHS_SHORT[month - 1].to_string());
                out.push_str(&name);
            }
            Token::Month2 => {
                let _ = write!(out, "{:02}", dt.month());
            }
            Token::Month1 => {
                let _ = write!(out, "{}", dt.month());
            }
            Token::Day2 => {
                let _ = write!(out, "{:02}", dt.day());
            }
            Token::Day1 => {
                let _ = write!(out, "{}", dt.day());
            }
            Token::WeekdayFull => {
                let w = dt.weekday().number_from_monday() as usize;
                let name = labels
                    .message(&format!("text-weekday-{w}"))
                    .unwrap_or_else(|| ENGLISH_WEEKDAYS[w - 1].to_string());
                out.push_str(&name);
            }
            Token::WeekdayShort => {
                let w = dt.weekday().number_from_monday() as usize;
                let name = labels
                    .message(&format!("text-weekday-{w}-short"))
                    .unwrap_or_else(|| ENGLISH_WEEKDAYS_SHORT[w - 1].to_string());
                out.push_str(&name);
            }
            Token::OffsetCol => {
                out.push_str(&dt.format("%:z").to_string());
            }
            Token::OffsetNoCol => {
                out.push_str(&dt.format("%z").to_string());
            }
            _ => {}
        }
        return;
    }

    let (hour, minute, second, nanosecond) = match moment {
        Moment::DateTime(dt) => (dt.hour(), dt.minute(), dt.second(), dt.nanosecond()),
        Moment::Time(t) => (t.hour(), t.minute(), t.second(), t.nanosecond()),
    };

    match token {
        Token::Hour24Two => {
            let _ = write!(out, "{hour:02}");
        }
        Token::Hour24One => {
            let _ = write!(out, "{hour}");
        }
        Token::Hour12Two => {
            let h12 = match hour % 12 {
                0 => 12,
                h => h,
            };
            let _ = write!(out, "{h12:02}");
        }
        Token::Hour12One => {
            let h12 = match hour % 12 {
                0 => 12,
                h => h,
            };
            let _ = write!(out, "{h12}");
        }
        Token::Minute2 => {
            let _ = write!(out, "{minute:02}");
        }
        Token::Minute1 => {
            let _ = write!(out, "{minute}");
        }
        Token::Second2 => {
            let _ = write!(out, "{second:02}");
        }
        Token::Second1 => {
            let _ = write!(out, "{second}");
        }
        Token::Millis => {
            let _ = write!(out, "{:03}", nanosecond / 1_000_000);
        }
        Token::AmPmUpper => {
            let text = if hour < 12 {
                labels
                    .message("text-am")
                    .unwrap_or_else(|| "AM".to_string())
            } else {
                labels
                    .message("text-pm")
                    .unwrap_or_else(|| "PM".to_string())
            };
            out.push_str(&text);
        }
        Token::AmPmLower => {
            let text = if hour < 12 {
                labels
                    .message("text-am")
                    .unwrap_or_else(|| "AM".to_string())
            } else {
                labels
                    .message("text-pm")
                    .unwrap_or_else(|| "PM".to_string())
            };
            out.push_str(&text.to_lowercase());
        }
        _ => {}
    }
}

fn format_pattern(
    moment: &Moment,
    pattern: &str,
    labels: &dyn QuantityLabels,
) -> Result<String, minijinja::Error> {
    let items = parse_pattern(pattern);

    if matches!(moment, Moment::Time(_)) {
        let has_date_token = items.iter().any(|item| match item {
            PatternItem::Token(t) => t.is_date_token(),
            _ => false,
        });
        if has_date_token {
            return Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                "date tokens need a date-time, not a time",
            ));
        }
    }

    let has_day_token = items.iter().any(|item| match item {
        PatternItem::Token(t) => t.is_day_token(),
        _ => false,
    });

    let mut out = String::new();
    for item in items {
        match item {
            PatternItem::Literal(s) => out.push_str(&s),
            PatternItem::Token(token) => {
                format_token(&mut out, token, moment, has_day_token, labels);
            }
        }
    }
    Ok(out)
}

fn date_filter(
    labels: &dyn QuantityLabels,
    value: Value,
    pattern: Option<String>,
) -> Result<Value, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(Value::UNDEFINED);
    }
    let Some(moment) = parse_moment(&value, "date")? else {
        return Ok(Value::UNDEFINED);
    };

    let pattern_str = match &pattern {
        Some(p) => p.as_str(),
        None => match &moment {
            Moment::DateTime(_) => "datetime",
            Moment::Time(_) => "time",
        },
    };

    if pattern_str == "iso" {
        return Ok(Value::from(match &moment {
            Moment::DateTime(dt) => dt.to_rfc3339(),
            Moment::Time(t) => t.format("%H:%M:%S").to_string(),
        }));
    }

    let resolved_pattern = match pattern_str {
        "datetime" => labels
            .message("text-date-format-datetime")
            .unwrap_or_else(|| "YYYY-MM-DD HH:mm".to_string()),
        "date" => labels
            .message("text-date-format-date")
            .unwrap_or_else(|| "YYYY-MM-DD".to_string()),
        "time" => labels
            .message("text-date-format-time")
            .unwrap_or_else(|| "HH:mm".to_string()),
        "long" => labels
            .message("text-date-format-long")
            .unwrap_or_else(|| "MMMM D, YYYY HH:mm".to_string()),
        other => other.to_string(),
    };

    let formatted = format_pattern(&moment, &resolved_pattern, labels)?;
    Ok(Value::from(formatted))
}

fn strftime_filter(value: Value, format_str: Option<String>) -> Result<Value, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(Value::UNDEFINED);
    }
    let format_str = format_str.ok_or_else(|| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            "strftime requires a format string argument",
        )
    })?;

    let Some(moment) = parse_moment(&value, "strftime")? else {
        return Ok(Value::UNDEFINED);
    };

    for item in StrftimeItems::new(&format_str) {
        if matches!(item, Item::Error) {
            return Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                "invalid strftime format",
            ));
        }
    }

    let mut output = String::new();
    let res = match moment {
        Moment::DateTime(dt) => write!(output, "{}", dt.format(&format_str)),
        Moment::Time(t) => write!(output, "{}", t.format(&format_str)),
    };
    res.map_err(|err| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("strftime formatting failed: {err}"),
        )
    })?;

    Ok(Value::from(output))
}

/// Formats a single date pattern or preset with a given local time and quantity labels.
pub fn example(
    pattern: &str,
    now: DateTime<chrono::Local>,
    labels: &dyn QuantityLabels,
) -> Option<String> {
    if pattern == "iso" {
        return Some(now.to_rfc3339());
    }
    let resolved_pattern = match pattern {
        "datetime" => labels
            .message("text-date-format-datetime")
            .unwrap_or_else(|| "YYYY-MM-DD HH:mm".to_string()),
        "date" => labels
            .message("text-date-format-date")
            .unwrap_or_else(|| "YYYY-MM-DD".to_string()),
        "time" => labels
            .message("text-date-format-time")
            .unwrap_or_else(|| "HH:mm".to_string()),
        "long" => labels
            .message("text-date-format-long")
            .unwrap_or_else(|| "MMMM D, YYYY HH:mm".to_string()),
        other => other.to_string(),
    };
    let moment = Moment::DateTime(now);
    format_pattern(&moment, &resolved_pattern, labels).ok()
}

pub(super) fn register(
    env: &mut Environment<'static>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) {
    let date_labels = Arc::clone(&labels);
    env.add_filter("date", move |value: Value, pattern: Option<String>| {
        date_filter(&*date_labels, value, pattern)
    });
    env.add_filter(
        "strftime",
        move |value: Value, format_str: Option<String>| strftime_filter(value, format_str),
    );
}
