use super::*;
use crate::{DataType, DataValue};
use chrono::{DateTime, Datelike, NaiveTime, Timelike};
use std::collections::BTreeMap;
use std::sync::Arc;

struct TestLabels {
    messages: BTreeMap<&'static str, &'static str>,
}

impl QuantityLabels for TestLabels {
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
    fn decimal_separator(&self) -> String {
        ".".into()
    }
    fn message(&self, id: &str) -> Option<String> {
        self.messages.get(id).map(|s| s.to_string())
    }
}

fn test_labels() -> Arc<dyn QuantityLabels + Send + Sync> {
    let mut messages = BTreeMap::new();
    messages.insert("text-date-format-datetime", "DD.MM.YYYY HH:mm");
    messages.insert("text-date-format-date", "DD.MM.YYYY");
    messages.insert("text-date-format-time", "HH:mm");
    messages.insert("text-date-format-long", "D MMMM YYYY, HH:mm");
    messages.insert("text-month-11", "листопад");
    messages.insert("text-month-11-format", "листопада");
    messages.insert("text-month-11-short", "лис");
    messages.insert("text-weekday-6", "субота");
    messages.insert("text-weekday-6-short", "сб");
    messages.insert("text-am", "дп");
    messages.insert("text-pm", "пп");
    Arc::new(TestLabels { messages })
}

fn render_template(
    source: &str,
    inputs: &[(String, Option<DataValue>, DataType)],
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) -> Result<String, crate::template::TemplateError> {
    let compiled = crate::template::CompiledTemplate::new(source)?;
    compiled.render_localized(
        inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
        labels,
    )
}

#[test]
fn test_undefined_input_renders_empty() {
    let labels = test_labels();
    let inputs = vec![("val".to_string(), None, DataType::DateTime)];
    let res = render_template("{{ val | date }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res, "");
    let res_strftime = render_template("{{ val | strftime('%Y') }}", &inputs, labels).unwrap();
    assert_eq!(res_strftime, "");
}

#[test]
fn test_time_presets_and_formatting() {
    let labels = test_labels();
    let time = NaiveTime::from_hms_milli_opt(14, 30, 45, 123).unwrap();
    let inputs = vec![("t".to_string(), Some(DataValue::Time(time)), DataType::Time)];

    // Default preset for time is "time" (HH:mm)
    let res = render_template("{{ t | date }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res, "14:30");

    // ISO preset for time is HH:MM:SS
    let res_iso = render_template("{{ t | date('iso') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_iso, "14:30:45");

    // Custom tokens with literal
    let res_tokens = render_template(
        "{{ t | date('HH:mm:ss [clock]') }}",
        &inputs,
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res_tokens, "14:30:45 clock");

    // 12-hour format with AM/PM
    let res_12h = render_template("{{ t | date('h:mm A / a') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_12h, "2:30 пп / пп");

    // Canonical English AM/PM
    let res_canonical = render_template(
        "{{ t | date('h:mm A / a') }}",
        &inputs,
        Arc::new(crate::quantity::CanonicalLabels),
    )
    .unwrap();
    assert_eq!(res_canonical, "2:30 PM / pm");
}

#[test]
fn test_date_token_on_time_errors() {
    let labels = test_labels();
    let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
    let inputs = vec![("t".to_string(), Some(DataValue::Time(time)), DataType::Time)];

    for pattern in [
        "YYYY", "YY", "MMMM", "MMM", "MM", "M", "DD", "D", "dddd", "ddd", "Z", "ZZ", "date",
        "datetime", "long",
    ] {
        let tmpl = format!("{{{{ t | date('{pattern}') }}}}");
        let err = render_template(&tmpl, &inputs, labels.clone()).unwrap_err();
        assert!(
            err.message
                .contains("date tokens need a date-time, not a time"),
            "Pattern '{pattern}' should fail on time: {}",
            err.message
        );
    }
}

#[test]
fn test_datetime_formatting_and_presets() {
    let labels = test_labels();
    let dt_str = "2026-11-14T15:30:45Z";
    let parsed = DateTime::parse_from_rfc3339(dt_str).unwrap();
    let local = parsed.with_timezone(&chrono::Local);

    let inputs = vec![(
        "dt".to_string(),
        Some(DataValue::String(dt_str.to_string())),
        DataType::DateTime,
    )];

    // iso preset
    let res_iso = render_template("{{ dt | date('iso') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_iso, local.to_rfc3339());

    // default datetime preset with test labels (DD.MM.YYYY HH:mm)
    let res_default = render_template("{{ dt | date }}", &inputs, labels.clone()).unwrap();
    let expected_default = format!(
        "{:02}.{:02}.{:04} {:02}:{:02}",
        local.day(),
        local.month(),
        local.year(),
        local.hour(),
        local.minute()
    );
    assert_eq!(res_default, expected_default);

    // date preset (DD.MM.YYYY)
    let res_date = render_template("{{ dt | date('date') }}", &inputs, labels.clone()).unwrap();
    let expected_date = format!(
        "{:02}.{:02}.{:04}",
        local.day(),
        local.month(),
        local.year()
    );
    assert_eq!(res_date, expected_date);

    // time preset (HH:mm)
    let res_time = render_template("{{ dt | date('time') }}", &inputs, labels.clone()).unwrap();
    let expected_time = format!("{:02}:{:02}", local.hour(), local.minute());
    assert_eq!(res_time, expected_time);
}

#[test]
fn test_format_vs_standalone_month() {
    let labels = test_labels();
    // 2026-11-14 is month 11 (листопад / листопада), day 14, weekday Saturday (субота / сб)
    let dt_str = "2026-11-14T10:00:00Z";
    let inputs = vec![(
        "dt".to_string(),
        Some(DataValue::String(dt_str.to_string())),
        DataType::DateTime,
    )];

    // Stand-alone MMMM (no D or DD token)
    let res_standalone =
        render_template("{{ dt | date('MMMM') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_standalone, "листопад");

    // Format-context MMMM (with D)
    let res_format_d =
        render_template("{{ dt | date('D MMMM') }}", &inputs, labels.clone()).unwrap();
    let expected_d = format!(
        "{} листопада",
        DateTime::parse_from_rfc3339(dt_str)
            .unwrap()
            .with_timezone(&chrono::Local)
            .day()
    );
    assert_eq!(res_format_d, expected_d);

    // Format-context MMMM (with DD)
    let res_format_dd =
        render_template("{{ dt | date('MMMM DD') }}", &inputs, labels.clone()).unwrap();
    let expected_dd = format!(
        "листопада {:02}",
        DateTime::parse_from_rfc3339(dt_str)
            .unwrap()
            .with_timezone(&chrono::Local)
            .day()
    );
    assert_eq!(res_format_dd, expected_dd);

    // Literal [Day] inside brackets must NOT trigger format context
    let res_literal =
        render_template("{{ dt | date('[Day] MMMM') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_literal, "Day листопад");

    // Short month and weekday
    let res_short = render_template("{{ dt | date('MMM, ddd (dddd)') }}", &inputs, labels).unwrap();
    assert_eq!(res_short, "лис, сб (субота)");
}

#[test]
fn test_tokens_and_brackets() {
    let labels = Arc::new(crate::quantity::CanonicalLabels);
    let dt_str = "2026-05-09T08:07:06.050Z";
    let parsed = DateTime::parse_from_rfc3339(dt_str).unwrap();
    let local = parsed.with_timezone(&chrono::Local);

    let inputs = vec![(
        "dt".to_string(),
        Some(DataValue::String(dt_str.to_string())),
        DataType::DateTime,
    )];

    let res = render_template(
        "{{ dt | date('YYYY/YY M/MM D/DD H/HH h/hh m/mm s/ss SSS [at] ZZ') }}",
        &inputs,
        labels,
    )
    .unwrap();

    let h12 = match local.hour() % 12 {
        0 => 12,
        h => h,
    };
    let expected = format!(
        "{:04}/{:02} {}/{:02} {}/{:02} {}/{:02} {}/{:02} {}/{:02} {}/{:02} {:03} at {}",
        local.year(),
        local.year().rem_euclid(100),
        local.month(),
        local.month(),
        local.day(),
        local.day(),
        local.hour(),
        local.hour(),
        h12,
        h12,
        local.minute(),
        local.minute(),
        local.second(),
        local.second(),
        local.nanosecond() / 1_000_000,
        local.format("%z")
    );
    assert_eq!(res, expected);
}

#[test]
fn test_unix_timestamp_input() {
    let labels = Arc::new(crate::quantity::CanonicalLabels);
    let inputs = vec![
        (
            "ts_int".to_string(),
            Some(DataValue::Number(crate::NumberValue::Int(1700000000))),
            DataType::Number,
        ),
        (
            "ts_float".to_string(),
            Some(DataValue::Number(crate::NumberValue::Float(1700000000.5))),
            DataType::Number,
        ),
    ];

    let dt = DateTime::from_timestamp(1700000000, 0)
        .unwrap()
        .with_timezone(&chrono::Local);
    let expected_year = format!("{:04}", dt.year());

    let res_int = render_template("{{ ts_int | date('YYYY') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_int, expected_year);

    let res_float = render_template("{{ ts_float | date('ss.SSS') }}", &inputs, labels).unwrap();
    assert_eq!(res_float, format!("{:02}.500", dt.second()));
}

#[test]
fn test_strftime_filter() {
    let labels = Arc::new(crate::quantity::CanonicalLabels);
    let dt_str = "2026-11-14T15:30:45Z";
    let parsed = DateTime::parse_from_rfc3339(dt_str).unwrap();
    let local = parsed.with_timezone(&chrono::Local);

    let time = NaiveTime::from_hms_opt(15, 30, 45).unwrap();

    let inputs = vec![
        (
            "dt".to_string(),
            Some(DataValue::String(dt_str.to_string())),
            DataType::DateTime,
        ),
        ("t".to_string(), Some(DataValue::Time(time)), DataType::Time),
    ];

    // strftime on datetime
    let res_dt = render_template(
        "{{ dt | strftime('%Y-%m-%d %H:%M:%S') }}",
        &inputs,
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res_dt, local.format("%Y-%m-%d %H:%M:%S").to_string());

    // strftime on time
    let res_t = render_template("{{ t | strftime('%H:%M:%S') }}", &inputs, labels.clone()).unwrap();
    assert_eq!(res_t, "15:30:45");

    // strftime %Y on time produces error, no panic
    let err_t = render_template("{{ t | strftime('%Y') }}", &inputs, labels.clone()).unwrap_err();
    assert!(
        err_t.message.contains("strftime formatting failed"),
        "{}",
        err_t.message
    );

    // invalid strftime format
    let err_invalid = render_template("{{ dt | strftime('%') }}", &inputs, labels).unwrap_err();
    assert!(
        err_invalid.message.contains("invalid strftime format"),
        "{}",
        err_invalid.message
    );
}
