//! Текстова форма значень, які можна набрати з клавіатури (поля на ноді, умови Match):
//! число, рядок, логічне, час, дата-час, тривалість, колір.
//!
//! - час — `HH:MM` або `HH:MM:SS`;
//! - дата-час — `YYYY-MM-DD HH:MM[:SS]` (UTC) або RFC 3339 зі зсувом;
//! - тривалість — секунди (`90`, `1.5`) або з одиницями: `1h 30m`, `45s`, `500ms` (також `год`, `хв`, `с`, `мс`);
//! - колір — `#rrggbb` або `#rrggbbaa`.

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

/// `90` / `1.5` — секунди; інакше послідовність «число одиниця»: `1h 30m`, `2хв`, `500ms`.
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
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        let number: f64 = chars[start..i].iter().collect::<String>().parse().ok()?;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let unit_start = i;
        while i < chars.len() && chars[i].is_alphabetic() {
            i += 1;
        }
        let unit: String = chars[unit_start..i].iter().collect::<String>().to_lowercase();
        let scale = match unit.as_str() {
            "ms" | "мс" => 0.001,
            "s" | "sec" | "с" | "сек" => 1.0,
            "m" | "min" | "хв" => 60.0,
            "h" | "год" => 3600.0,
            "d" | "дн" | "д" => 86_400.0,
            _ => return None,
        };
        total += number * scale;
        parts += 1;
    }
    if parts == 0 {
        return None;
    }
    Duration::try_from_secs_f64(total).ok()
}

pub fn parse_color(text: &str) -> Option<Color> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.is_ascii() || !(hex.len() == 6 || hex.len() == 8) {
        return None;
    }
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
    // Частка секунди йде в останню частину: `1m 1.5s`, а не `1m 1s`
    let nanos = d.subsec_nanos();
    if nanos != 0 {
        parts.push(format!("{}s", secs as f64 + f64::from(nanos) / 1e9));
    } else if secs > 0 {
        parts.push(format!("{secs}s"));
    }
    parts.join(" ")
}

/// Текст значення для поля вводу (без лапок у рядків). `parse_value` розбирає його назад.
pub fn format_value(value: &DataValue) -> String {
    match value {
        DataValue::String(s) => s.clone(),
        DataValue::Number(NumberValue::Int(i)) => i.to_string(),
        DataValue::Number(NumberValue::Float(f)) => f.to_string(),
        DataValue::Bool(b) => b.to_string(),
        DataValue::Time(t) => t.format(if t.format("%S").to_string() == "00" { "%H:%M" } else { "%H:%M:%S" }).to_string(),
        DataValue::DateTime(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        DataValue::Duration(d) => format_duration(*d),
        DataValue::Color(c) if c.a == 255 => format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b),
        DataValue::Color(c) => format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => format!("[{}]", elements.iter().map(format_value).collect::<Vec<_>>().join(", ")),
        DataValue::Object { fields, .. } => {
            let fields: Vec<_> = fields.iter().map(|(k, f)| format!("{k}: {}", format_value(&f.value))).collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

/// Значення невідомого типу: число, `true`/`false`, інакше рядок (лапки дозволяють записати
/// рядок `"5"`).
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

/// Чи можна задати значення цього типу текстом.
pub fn is_text_type(t: &DataType) -> bool {
    matches!(
        t,
        DataType::Number | DataType::String | DataType::Bool | DataType::Time | DataType::DateTime
            | DataType::Duration | DataType::Color | DataType::Any
    )
}

/// Як `is_text_type`, але значення переліку теж задаються (вибором зі списку або текстом).
pub fn is_text_type_in(types: &TypeRegistry, t: &DataType) -> bool {
    is_text_type(t) || types.definition_of(t).is_some_and(|d| d.is_enum())
}

/// Підказка для поля вводу значення цього типу.
pub fn type_hint(t: &DataType) -> &'static str {
    match t {
        DataType::Time => "ГГ:ХХ",
        DataType::DateTime => "РРРР-ММ-ДД ГГ:ХХ",
        DataType::Duration => "1h 30m / 45s",
        DataType::Color => "#rrggbb",
        _ => "—",
    }
}

/// Типове значення для нового поля типу `t` (Any — число 0).
pub fn default_of(t: &DataType) -> DataValue {
    match t {
        DataType::String => DataValue::String(String::new()),
        DataType::Bool => DataValue::Bool(true),
        DataType::Time => DataValue::Time(NaiveTime::MIN),
        DataType::DateTime => DataValue::DateTime(DateTime::<Utc>::UNIX_EPOCH),
        DataType::Duration => DataValue::Duration(Duration::from_secs(60)),
        DataType::Color => DataValue::Color(Color { r: 255, g: 255, b: 255, a: 255 }),
        DataType::List(inner) => DataValue::List { element_type: (**inner).clone(), elements: Vec::new() },
        DataType::Object | DataType::Named(_) => DataValue::Object { fields: Default::default(), type_name: None },
        DataType::Number | DataType::Any => DataValue::Number(NumberValue::Int(0)),
    }
}

/// Розбирає текст як значення типу `t`.
pub fn parse_value(text: &str, t: &DataType) -> Result<DataValue, String> {
    let bad = |what: &str| format!("«{}» — не {what}", text.trim());
    Ok(match t {
        DataType::Number => DataValue::Number(parse_number(text).ok_or_else(|| bad("число"))?),
        DataType::String => DataValue::String(text.to_string()),
        DataType::Bool => match text.trim() {
            "true" | "так" | "1" => DataValue::Bool(true),
            "false" | "ні" | "0" => DataValue::Bool(false),
            _ => return Err(bad("true/false")),
        },
        DataType::Time => DataValue::Time(parse_time(text).ok_or_else(|| bad("час (ГГ:ХХ або ГГ:ХХ:СС)"))?),
        DataType::DateTime => DataValue::DateTime(parse_datetime(text).ok_or_else(|| bad("дата-час (РРРР-ММ-ДД ГГ:ХХ)"))?),
        DataType::Duration => DataValue::Duration(parse_duration(text).ok_or_else(|| bad("тривалість (90, 1h 30m, 500ms)"))?),
        DataType::Color => DataValue::Color(parse_color(text).ok_or_else(|| bad("колір (#rrggbb)"))?),
        DataType::Any => parse_any(text),
        DataType::List(_) | DataType::Object | DataType::Named(_) => return Err(format!("значення типу {} не набирається текстом", crate::data_type_name(t))),
    })
}

/// Як `parse_value`, але знає власні типи: значення переліку — рядок з переліку (за значенням
/// або за підписом), структура текстом не набирається.
pub fn parse_value_in(types: &TypeRegistry, text: &str, t: &DataType) -> Result<DataValue, String> {
    match types.definition_of(t) {
        Some(def) if def.is_enum() => {
            let wanted = text.trim();
            def.values().iter()
                .find(|v| v.value == wanted)
                .or_else(|| def.values().iter().find(|v| v.title == wanted))
                .map(|v| DataValue::String(v.value.clone()))
                .ok_or_else(|| {
                    let all: Vec<_> = def.values().iter().map(|v| v.value.as_str()).collect();
                    format!("«{wanted}» — не значення {} (допустимі: {})", def.title, all.join(", "))
                })
        }
        Some(def) => Err(format!("значення типу {} не набирається текстом", def.title)),
        None => parse_value(text, t),
    }
}

/// Приводить `value` до типу `like` там, де це однозначно: рядок → час / дата / колір / число,
/// число → тривалість (секунди). Інакше повертає як є.
pub fn coerce_to(value: DataValue, like: &DataType) -> DataValue {
    match (&value, like) {
        (DataValue::String(s), DataType::Number | DataType::Bool | DataType::Time | DataType::DateTime | DataType::Duration | DataType::Color) => {
            parse_value(s, like).unwrap_or(value)
        }
        // Нескінченність, від'ємне чи завелике число лишаються числом — порівняння дасть помилку типу
        (DataValue::Number(n), DataType::Duration) => match Duration::try_from_secs_f64(n.to_f64()) {
            Ok(d) => DataValue::Duration(d),
            Err(_) => value,
        },
        _ => value,
    }
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
        // Нескінченність і завеликі числа не перетворюються (і не панікують)
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
