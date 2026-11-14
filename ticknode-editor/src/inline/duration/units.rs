//! Duration parsing and lossless number/unit presentation, independent of the DOM.

use crate::i18n::t;
use ticknode_shared::{DataValue, UiMessage, text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Ticks,
    Milliseconds,
    Seconds,
    Minutes,
    Hours,
}

impl Unit {
    pub const ALL: [Self; 5] = [
        Self::Ticks,
        Self::Milliseconds,
        Self::Seconds,
        Self::Minutes,
        Self::Hours,
    ];

    pub fn available(wall_only: bool) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |unit| !wall_only || *unit != Self::Ticks)
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Ticks => "t",
            Self::Milliseconds => "ms",
            Self::Seconds => "s",
            Self::Minutes => "min",
            Self::Hours => "h",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|u| u.id() == id)
    }

    pub fn label(self) -> String {
        match self {
            Self::Ticks => t!("duration-unit-ticks"),
            Self::Milliseconds => t!("duration-unit-ms"),
            Self::Seconds => t!("duration-unit-s"),
            Self::Minutes => t!("duration-unit-min"),
            Self::Hours => t!("duration-unit-h"),
        }
    }

    fn nanos(self) -> Option<u128> {
        match self {
            Self::Ticks => None,
            Self::Milliseconds => Some(1_000_000),
            Self::Seconds => Some(1_000_000_000),
            Self::Minutes => Some(60_000_000_000),
            Self::Hours => Some(3_600_000_000_000),
        }
    }

    fn whole(self, value: &DataValue) -> Option<String> {
        match value {
            DataValue::Ticks(n) if self == Self::Ticks => Some(n.to_string()),
            DataValue::Duration(d) => {
                let size = self.nanos()? as i128;
                let total_nanos =
                    (d.num_seconds() as i128) * 1_000_000_000 + (d.subsec_nanos() as i128);
                (total_nanos % size == 0).then(|| (total_nanos / size).to_string())
            }
            _ => None,
        }
    }
}

/// Keep a selected unit if it can represent the value exactly; otherwise choose the largest whole unit.
pub(crate) fn presentation(value: Option<&DataValue>, preferred: Option<Unit>) -> (String, Unit) {
    let Some(value) = value else {
        return (String::new(), preferred.unwrap_or(Unit::Seconds));
    };
    for unit in preferred.into_iter().chain([
        Unit::Ticks,
        Unit::Hours,
        Unit::Minutes,
        Unit::Seconds,
        Unit::Milliseconds,
    ]) {
        if let Some(number) = unit.whole(value) {
            return (number, unit);
        }
    }
    // The shared formatter truncates sub-millisecond values; keep them exact in an editable field.
    let text = match value {
        DataValue::Duration(d) => {
            let num_secs = d.num_seconds();
            let subsec = d.subsec_nanos();
            if *d < ticknode_shared::TimeDelta::zero() {
                format!("-{}.{:09}s", num_secs.abs(), subsec.abs())
            } else {
                format!("{num_secs}.{subsec:09}s")
            }
        }
        _ => text::format_value(value),
    };
    (text, preferred.unwrap_or(Unit::Seconds))
}

pub(crate) fn parse(input: &str, unit: Unit) -> Result<Option<DataValue>, UiMessage> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    let normalized = input.replace(',', ".");
    let parsed = if normalized.parse::<f64>().is_ok() {
        if unit == Unit::Ticks {
            input.parse::<u64>().ok().map(DataValue::Ticks)
        } else if let Some(unit_nanos) = unit.nanos() {
            if let Ok(number) = input.parse::<i128>() {
                // Integer input avoids rounding large values through f64.
                let size = unit_nanos as i128;
                number.checked_mul(size).and_then(|nanos| {
                    let secs = nanos.div_euclid(1_000_000_000);
                    let sub_nanos = nanos.rem_euclid(1_000_000_000) as u32;
                    ticknode_shared::delta_serde::combine_delta(
                        i64::try_from(secs).ok()?,
                        sub_nanos,
                    )
                    .map(DataValue::Duration)
                })
            } else {
                normalized
                    .parse::<f64>()
                    .ok()
                    .and_then(|n| ticknode_shared::delta_from_secs_f64(n * unit_nanos as f64 / 1e9))
                    .map(DataValue::Duration)
            }
        } else {
            None
        }
    } else {
        text::parse_duration_value(input)
    };
    parsed.map(Some).ok_or_else(|| {
        UiMessage::new(
            "parse-error-duration",
            format!("\"{input}\" is not a duration (10t, 1h 30m, 500ms)"),
        )
        .arg("value", input)
    })
}

#[cfg(test)]
mod tests;

/// Schema duration properties accept clock text and wall-clock units only.
pub(crate) fn parse_property(input: &str, unit: Unit) -> Result<Option<DataValue>, UiMessage> {
    let invalid = || UiMessage::new("error-property-duration", "Invalid duration property value");
    let parsed = if input.contains(':') {
        ticknode_shared::property::parse_property_duration(input)
            .map(DataValue::Duration)
            .map(Some)
            .ok_or_else(invalid)
    } else {
        parse(input, unit).map_err(|_| invalid())
    }?;
    match parsed {
        Some(DataValue::Ticks(_)) => Err(invalid()),
        Some(DataValue::Duration(d)) if d < ticknode_shared::TimeDelta::zero() => Err(invalid()),
        _ => Ok(parsed),
    }
}
