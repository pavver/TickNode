//! Local condition evaluation and rendered-template truth values.

use super::Kind;
use crate::live::Live;
use chrono::{DateTime, Datelike, NaiveTime, TimeDelta, Utc};
use serde_json::Value;
use std::collections::BTreeMap;

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
    let evaluation = Evaluation {
        properties: p,
        live,
        now,
    };
    match kind {
        Kind::State => evaluation.state(),
        Kind::NumericState => evaluation.numeric_state(),
        Kind::Time => evaluation.time(),
        Kind::Sun => evaluation.sun(),
        Kind::Zone => evaluation.zone(),
        Kind::Template => Err("Template conditions require asynchronous evaluation".into()),
    }
}

struct Evaluation<'a> {
    properties: &'a BTreeMap<String, String>,
    live: &'a Live,
    now: DateTime<Utc>,
}
impl Evaluation<'_> {
    fn p(&self, key: &str) -> &str {
        self.properties
            .get(key)
            .map(String::as_str)
            .unwrap_or_default()
    }
    fn value(&self, entity: &str, attr: &str) -> Result<Value, String> {
        let state = self.live.state(entity)?;
        if attr.is_empty() {
            Ok(Value::String(state.state.clone()))
        } else {
            state
                .attributes
                .get(attr)
                .ok_or_else(|| format!("Entity {entity} has no attribute {attr}"))
        }
    }
    fn state(&self) -> Result<bool, String> {
        let actual = scalar(&self.value(self.p("entity_id"), self.p("attribute"))?);
        let held = if self.p("for").is_empty() {
            true
        } else {
            if self.p("for").starts_with('-') {
                return Err("For must not be negative".into());
            }
            let hold = offset(self.p("for"))?;
            self.now
                .signed_duration_since(self.live.state(self.p("entity_id"))?.last_changed)
                >= hold
        };
        Ok(held
            && self
                .p("state")
                .split(',')
                .map(str::trim)
                .any(|expected| expected == actual))
    }

    fn numeric_state(&self) -> Result<bool, String> {
        let actual = number(&self.value(self.p("entity_id"), self.p("attribute"))?)
            .ok_or("Condition value is not a finite number")?;

        let above = self.bound(self.p("above"))?;
        let below = self.bound(self.p("below"))?;
        if above.is_none() && below.is_none() {
            return Err("Numeric condition needs Above or Below".into());
        }
        Ok(above.is_none_or(|n| actual > n) && below.is_none_or(|n| actual < n))
    }

    fn bound(&self, text: &str) -> Result<Option<f64>, String> {
        if text.is_empty() {
            return Ok(None);
        }
        if let Ok(n) = text.parse::<f64>() {
            return n
                .is_finite()
                .then_some(Some(n))
                .ok_or_else(|| "Numeric bound must be finite".into());
        }
        number(&self.value(text, "")?)
            .map(Some)
            .ok_or_else(|| format!("Bound entity {text} is not numeric"))
    }

    fn time(&self) -> Result<bool, String> {
        let local = self.now.with_timezone(&self.live.offset()?);
        let after = clock(self.p("after"))?;
        let before = clock(self.p("before"))?;
        let weekdays = weekdays(self.p("weekday"))?;
        let time = local.time();
        let within = within_time(time, after, before);
        Ok(within
            && (weekdays.is_empty()
                || weekdays.contains(&(local.weekday().num_days_from_monday() as usize))))
    }

    fn sun(&self) -> Result<bool, String> {
        let local = self.now.with_timezone(&self.live.offset()?);
        let after = self.solar_boundary(self.p("after"), self.p("after_offset"))?;
        let before = self.solar_boundary(self.p("before"), self.p("before_offset"))?;
        Ok(after.is_none_or(|a| local >= a) && before.is_none_or(|b| local <= b))
    }

    fn solar_boundary(
        &self,
        event: &str,
        delta: &str,
    ) -> Result<Option<DateTime<chrono::FixedOffset>>, String> {
        let zone = self.live.offset()?;
        let local = self.now.with_timezone(&zone);

        let Some(attr) = solar_attribute(event)? else {
            return Ok(None);
        };
        let next = self.solar_event(attr)?;
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
    }

    fn solar_event(&self, attr: &str) -> Result<DateTime<chrono::FixedOffset>, String> {
        let next = self.value("sun.sun", attr)?;
        let next = DateTime::parse_from_rfc3339(next.as_str().ok_or("Invalid solar timestamp")?)
            .map_err(|_| "Invalid solar timestamp")?;
        if (next.with_timezone(&Utc) - self.now).num_seconds().abs() > 2 * 86400 {
            return Err("Solar event is too far away (polar day/night)".into());
        }
        Ok(next)
    }
    fn zone(&self) -> Result<bool, String> {
        let person = self.live.state(self.p("entity_id"))?;
        let zone = self.live.state(self.p("zone"))?;
        let name = if self.p("zone") == "zone.home" {
            "home".to_owned()
        } else {
            zone.attributes
                .get("friendly_name")
                .and_then(|v| v.as_str().map(str::to_owned))
                .ok_or("Zone has no friendly_name")?
        };
        Ok(person.state == name)
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

fn weekdays(text: &str) -> Result<Vec<usize>, String> {
    let mut weekdays = Vec::new();
    for day in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        weekdays.push(
            ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                .iter()
                .position(|s| s.eq_ignore_ascii_case(day))
                .ok_or_else(|| format!("Invalid weekday: {day}"))?,
        );
    }
    Ok(weekdays)
}

fn within_time(time: NaiveTime, after: Option<NaiveTime>, before: Option<NaiveTime>) -> bool {
    match (after, before) {
        (Some(a), Some(b)) if a >= b => time >= a || time < b,
        (a, b) => a.is_none_or(|a| time >= a) && b.is_none_or(|b| time < b),
    }
}

fn solar_attribute(event: &str) -> Result<Option<&'static str>, String> {
    match event {
        "none" | "" => Ok(None),
        "sunrise" => Ok(Some("next_rising")),
        "sunset" => Ok(Some("next_setting")),
        _ => Err("Invalid sun event".into()),
    }
}
