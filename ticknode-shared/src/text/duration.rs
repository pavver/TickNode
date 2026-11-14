use chrono::TimeDelta;

use crate::{DataValue, delta_from_secs_f64};

/// `90` / `1.5` — seconds; otherwise a sequence of "number unit" pairs: `1h 30m`, `45s`, `500ms`.
/// Accepts an optional leading `-` or `−` for negative durations.
pub fn parse_duration(text: &str) -> Option<TimeDelta> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let (is_negative, rest) =
        if let Some(stripped) = t.strip_prefix('-').or_else(|| t.strip_prefix('\u{2212}')) {
            (true, stripped.trim_start())
        } else {
            (false, t)
        };
    if rest.is_empty() || rest.starts_with(['-', '+', '\u{2212}']) {
        return None;
    }
    let normalized = rest.replace(',', ".");
    if let Ok(secs) = normalized.parse::<f64>() {
        if secs < 0.0 {
            return None;
        }
        let total = if is_negative { -secs } else { secs };
        return delta_from_secs_f64(total);
    }
    let chars: Vec<char> = normalized.chars().collect();
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
    if parts == 0 || i != chars.len() {
        return None;
    }
    let total = if is_negative { -total } else { total };
    delta_from_secs_f64(total)
}

/// Parses a tick count with an English or Ukrainian suffix, or a wall-clock duration.
pub fn parse_duration_value(text: &str) -> Option<DataValue> {
    let lower = text.trim().to_lowercase();
    for suffix in ["ticks", "tick", "t", "тактів", "такти", "такт"] {
        if let Some(number) = lower.strip_suffix(suffix) {
            let number = number.trim();
            return (!number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
                .then(|| number.parse().ok())
                .flatten()
                .map(DataValue::Ticks);
        }
    }
    parse_duration(text).map(DataValue::Duration)
}

fn duration_part(chars: &[char], position: &mut usize) -> Option<f64> {
    let mut i = *position;
    let start = i;
    while chars
        .get(i)
        .is_some_and(|ch| ch.is_ascii_digit() || *ch == '.')
    {
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
    let unit: String = chars[unit_start..i]
        .iter()
        .collect::<String>()
        .to_lowercase();
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

pub fn format_duration(d: TimeDelta) -> String {
    if d < TimeDelta::zero() {
        match TimeDelta::zero().checked_sub(&d) {
            Some(pos) => format!("-{}", format_positive_duration(pos)),
            None => format!("-{}", format_positive_duration(TimeDelta::MAX)),
        }
    } else {
        format_positive_duration(d)
    }
}

fn format_positive_duration(d: TimeDelta) -> String {
    let secs_total = d.num_seconds().max(0);
    let nanos = d.subsec_nanos().max(0) as u32;
    let ms = if secs_total == 0 {
        nanos as i64 / 1_000_000
    } else {
        1000
    };
    if secs_total == 0 && ms == 0 {
        return "0s".into();
    }
    if secs_total == 0 && ms < 1000 {
        return format!("{ms}ms");
    }
    let mut secs = secs_total as u64;
    let mut parts = Vec::new();
    for (unit, size) in [("d", 86_400), ("h", 3600), ("m", 60)] {
        if secs >= size {
            parts.push(format!("{}{unit}", secs / size));
            secs %= size;
        }
    }
    // Subsecond fraction goes into the last part: `1m 1.5s`, not `1m 1s`
    if nanos != 0 {
        parts.push(format!("{}s", secs as f64 + f64::from(nanos) / 1e9));
    } else if secs > 0 {
        parts.push(format!("{secs}s"));
    }
    parts.join(" ")
}
