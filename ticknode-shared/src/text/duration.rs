use std::time::Duration;

use crate::DataValue;

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
