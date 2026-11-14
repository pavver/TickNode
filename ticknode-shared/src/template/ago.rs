//! Relative time filter (`ago`) for MiniJinja templates.

use crate::plural;
use crate::quantity::QuantityLabels;
use crate::template::dates;
use chrono::{DateTime, TimeDelta, Utc};
use minijinja::{Environment, ErrorKind, Value};
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub fn ago_filter(
    labels: &dyn QuantityLabels,
    now: DateTime<Utc>,
    value: Value,
) -> Result<Value, minijinja::Error> {
    if value.is_undefined() || value.is_none() {
        return Ok(Value::UNDEFINED);
    }
    let Some(moment) = dates::parse_moment(&value, "ago")? else {
        return Ok(Value::UNDEFINED);
    };
    let t = match moment {
        dates::Moment::DateTime(dt) => dt.with_timezone(&Utc),
        dates::Moment::Time(_) => {
            return Err(minijinja::Error::new(
                ErrorKind::InvalidOperation,
                "ago needs a date-time, not a time",
            ));
        }
    };

    let d = now.signed_duration_since(t);
    let abs_d = if d < TimeDelta::zero() { -d } else { d };
    let abs_secs_f64 = abs_d.num_milliseconds() as f64 / 1000.0;

    if abs_d < TimeDelta::seconds(45) {
        let text = labels
            .message("text-ago-now")
            .unwrap_or_else(|| "just now".to_string());
        return Ok(Value::from(text));
    }

    let direction = if d >= TimeDelta::zero() {
        "past"
    } else {
        "future"
    };

    let (unit, n, unit_sing, unit_plur) = if abs_d < TimeDelta::minutes(45) {
        let n = (abs_secs_f64 / 60.0).round() as i64;
        ("minutes", n.max(1), "minute", "minutes")
    } else if abs_d < TimeDelta::hours(22) {
        let n = (abs_secs_f64 / 3600.0).round() as i64;
        ("hours", n.max(1), "hour", "hours")
    } else if abs_d < TimeDelta::days(26) {
        let n = (abs_secs_f64 / 86_400.0).round() as i64;
        ("days", n.max(1), "day", "days")
    } else if abs_d < TimeDelta::days(320) {
        let n = (abs_secs_f64 / (30.44 * 86_400.0)).round() as i64;
        ("months", n.max(1), "month", "months")
    } else {
        let n = (abs_secs_f64 / (365.25 * 86_400.0)).round() as i64;
        ("years", n.max(1), "year", "years")
    };

    let cat = plural::category(&labels.locale(), n as f64);
    let key = format!("text-ago-{direction}-{unit}-{}", cat.as_str());
    let n_str = n.to_string();
    let text = labels
        .message_args(&key, &[("n", n_str.clone())])
        .or_else(|| {
            let fb_key = format!("text-ago-{direction}-{unit}-other");
            labels.message_args(&fb_key, &[("n", n_str.clone())])
        })
        .unwrap_or_else(|| {
            if direction == "past" {
                if n == 1 {
                    format!("1 {unit_sing} ago")
                } else {
                    format!("{n} {unit_plur} ago")
                }
            } else if n == 1 {
                format!("in 1 {unit_sing}")
            } else {
                format!("in {n} {unit_plur}")
            }
        });

    Ok(Value::from(text))
}

pub(super) fn register(
    env: &mut Environment<'static>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
    now: DateTime<Utc>,
) {
    let ago_labels = Arc::clone(&labels);
    env.add_filter("ago", move |value: Value| {
        ago_filter(&*ago_labels, now, value)
    });
}
