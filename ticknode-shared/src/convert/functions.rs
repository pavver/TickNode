use crate::{Color, DataValue};
use chrono::{DateTime, Local};
use std::time::Duration;

pub(super) fn number(value: &DataValue) -> Option<f64> {
    let DataValue::Number(n) = value else {
        return None;
    };
    let n = n.to_f64();
    n.is_finite().then_some(n)
}

pub(super) fn finite(n: f64) -> Option<DataValue> {
    n.is_finite().then(|| n.into())
}

pub(super) fn beaufort(value: &DataValue) -> Option<DataValue> {
    let v = number(value)?;
    finite(
        (v.clamp(0.0, 1000.0) / 0.836)
            .powf(2.0 / 3.0)
            .round()
            .clamp(0.0, 12.0),
    )
}

pub(super) fn speed(value: &DataValue) -> Option<DataValue> {
    let b = number(value)?;
    finite(0.836 * b.powf(1.5))
}

pub(super) fn power(value: &DataValue) -> Option<DataValue> {
    finite(10f64.powf(number(value)? / 10.0 - 3.0))
}

pub(super) fn signal_strength(value: &DataValue) -> Option<DataValue> {
    let w = number(value)?;
    if w <= 0.0 {
        return None;
    }
    // Separating the logarithms avoids overflowing W * 1000 for finite inputs.
    finite(10.0 * (w.log10() + 3.0))
}

pub(super) fn color(value: &DataValue) -> Option<DataValue> {
    let t = number(value)?.clamp(1000.0, 40000.0) / 100.0;
    // Tanner Helland's approximation, with channel values clipped to sRGB bytes.
    let r = if t <= 66.0 {
        255.0
    } else {
        329.698727446 * (t - 60.0).powf(-0.1332047592)
    };
    let g = if t <= 66.0 {
        99.4708025861 * t.ln() - 161.1195681661
    } else {
        288.1221695283 * (t - 60.0).powf(-0.0755148492)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.5177312231 * (t - 10.0).ln() - 305.0447927307
    };
    let channel = |v: f64| v.clamp(0.0, 255.0).round() as u8;
    Some(DataValue::Color(Color {
        r: channel(r),
        g: channel(g),
        b: channel(b),
        a: 255,
    }))
}

pub(super) fn time(value: &DataValue) -> Option<DataValue> {
    let DataValue::DateTime(dt) = value else {
        return None;
    };
    Some(DataValue::Time(dt.with_timezone(&Local).time()))
}

pub(super) fn unix_seconds(value: &DataValue) -> Option<DataValue> {
    let DataValue::DateTime(dt) = value else {
        return None;
    };
    finite(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1e9)
}

pub(super) fn datetime(value: &DataValue) -> Option<DataValue> {
    let n = number(value)?;
    let seconds = n.floor();
    if seconds < i64::MIN as f64 || seconds >= i64::MAX as f64 {
        return None;
    }
    let nanos = ((n - seconds) * 1e9).round() as u32;
    let (seconds, nanos) = if nanos == 1_000_000_000 {
        ((seconds as i64).checked_add(1)?, 0)
    } else {
        (seconds as i64, nanos)
    };
    DateTime::from_timestamp(seconds, nanos).map(DataValue::DateTime)
}

pub(super) fn seconds(value: &DataValue) -> Option<DataValue> {
    let DataValue::Duration(d) = value else {
        return None;
    };
    finite(d.as_secs_f64())
}

pub(super) fn duration(value: &DataValue) -> Option<DataValue> {
    Duration::try_from_secs_f64(number(value)?)
        .ok()
        .map(DataValue::Duration)
}
