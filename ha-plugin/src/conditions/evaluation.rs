//! Local condition evaluation and rendered-template truth values.

use std::collections::BTreeMap;
use chrono::{DateTime, Datelike, NaiveTime, TimeDelta, Utc};
use serde_json::Value;
use crate::live::Live;
use super::Kind;

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}
fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn offset(text: &str) -> Result<TimeDelta, String> {
    if text.is_empty() {
        return Ok(TimeDelta::zero());
    }
    let (sign, text) = text
        .strip_prefix('-')
        .map_or((1, text.strip_prefix('+').unwrap_or(text)), |s| (-1, s));
    crate::triggers::duration(text)
        .and_then(|d| TimeDelta::from_std(d).ok())
        .map(|d| d * sign)
        .ok_or_else(|| "Invalid duration".into())
}
fn clock(text: &str) -> Result<Option<NaiveTime>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    NaiveTime::parse_from_str(text, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M"))
        .map(Some)
        .map_err(|_| format!("Invalid time: {text}"))
}

pub fn evaluate(
    kind: Kind,
    p: &BTreeMap<String, String>,
    live: &Live,
    now: DateTime<Utc>,
) -> Result<bool, String> {
    let p = |key: &str| p.get(key).map(String::as_str).unwrap_or_default();
    let value = |entity: &str, attr: &str| -> Result<Value, String> {
        let state = live.state(entity)?;
        if attr.is_empty() {
            Ok(Value::String(state.state.clone()))
        } else {
            state
                .attributes
                .get(attr)
                .ok_or_else(|| format!("Entity {entity} has no attribute {attr}"))
        }
    };
    match kind {
        Kind::State => {
            let actual = scalar(&value(p("entity_id"), p("attribute"))?);
            let held = if p("for").is_empty() {
                true
            } else {
                if p("for").starts_with('-') {
                    return Err("For must not be negative".into());
                }
                let hold = offset(p("for"))?;
                now.signed_duration_since(live.state(p("entity_id"))?.last_changed) >= hold
            };
            Ok(held
                && p("state")
                    .split(',')
                    .map(str::trim)
                    .any(|expected| expected == actual))
        }
        Kind::NumericState => {
            let actual = number(&value(p("entity_id"), p("attribute"))?)
                .ok_or("Condition value is not a finite number")?;
            let bound = |text: &str| -> Result<Option<f64>, String> {
                if text.is_empty() {
                    return Ok(None);
                }
                if let Ok(n) = text.parse::<f64>() {
                    return n
                        .is_finite()
                        .then_some(Some(n))
                        .ok_or_else(|| "Numeric bound must be finite".into());
                }
                number(&value(text, "")?)
                    .map(Some)
                    .ok_or_else(|| format!("Bound entity {text} is not numeric"))
            };
            let above = bound(p("above"))?;
            let below = bound(p("below"))?;
            if above.is_none() && below.is_none() {
                return Err("Numeric condition needs Above or Below".into());
            }
            Ok(above.is_none_or(|n| actual > n) && below.is_none_or(|n| actual < n))
        }
        Kind::Time => {
            let local = now.with_timezone(&live.offset()?);
            let after = clock(p("after"))?;
            let before = clock(p("before"))?;
            let mut weekdays = Vec::new();
            for day in p("weekday")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                weekdays.push(
                    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                        .iter()
                        .position(|s| s.eq_ignore_ascii_case(day))
                        .ok_or_else(|| format!("Invalid weekday: {day}"))?,
                );
            }
            let time = local.time();
            let within = match (after, before) {
                (Some(a), Some(b)) if a >= b => time >= a || time < b,
                (a, b) => a.is_none_or(|a| time >= a) && b.is_none_or(|b| time < b),
            };
            Ok(within
                && (weekdays.is_empty()
                    || weekdays.contains(&(local.weekday().num_days_from_monday() as usize))))
        }
        Kind::Sun => {
            let zone = live.offset()?;
            let local = now.with_timezone(&zone);
            let boundary = |event: &str,
                            delta: &str|
             -> Result<Option<DateTime<chrono::FixedOffset>>, String> {
                if event == "none" || event.is_empty() {
                    return Ok(None);
                }
                let attr = match event {
                    "sunrise" => "next_rising",
                    "sunset" => "next_setting",
                    _ => return Err("Invalid sun event".into()),
                };
                let next = value("sun.sun", attr)?;
                let next =
                    DateTime::parse_from_rfc3339(next.as_str().ok_or("Invalid solar timestamp")?)
                        .map_err(|_| "Invalid solar timestamp")?;
                if (next.with_timezone(&Utc) - now).num_seconds().abs() > 2 * 86400 {
                    return Err("Solar event is too far away (polar day/night)".into());
                }
                let today = local
                    .date_naive()
                    .and_time(next.with_timezone(&zone).time())
                    .and_local_timezone(zone)
                    .single()
                    .ok_or("Invalid solar date")?;
                Ok(Some(
                    today
                        .checked_add_signed(offset(delta)?)
                        .ok_or("Solar offset is out of range")?,
                ))
            };
            let after = boundary(p("after"), p("after_offset"))?;
            let before = boundary(p("before"), p("before_offset"))?;
            Ok(after.is_none_or(|a| local >= a) && before.is_none_or(|b| local <= b))
        }
        Kind::Zone => {
            let person = live.state(p("entity_id"))?;
            let zone = live.state(p("zone"))?;
            let name = if p("zone") == "zone.home" {
                "home".to_owned()
            } else {
                zone.attributes
                    .get("friendly_name")
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .ok_or("Zone has no friendly_name")?
            };
            Ok(person.state == name)
        }
        Kind::Template => Err("Template conditions require asynchronous evaluation".into()),
    }
}

pub fn template_bool(value: &Value) -> bool {
    // HA template conditions accept only the rendered text "true" (case-insensitive).
    value.as_bool().unwrap_or_else(|| {
        value
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("true"))
    })
}
