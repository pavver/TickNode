use crate::quantity::QuantityLabels;
use crate::template::{CompiledTemplate, RenderEnv};
use crate::{DataType, DataValue};
use chrono::{TimeDelta, Utc};
use std::collections::HashMap;
use std::sync::Arc;

struct TestDurationLabels {
    messages: HashMap<String, String>,
}

impl TestDurationLabels {
    fn uk() -> Self {
        let mut m = HashMap::new();
        m.insert("text-duration-d".into(), "д".into());
        m.insert("text-duration-h".into(), "год".into());
        m.insert("text-duration-m".into(), "хв".into());
        m.insert("text-duration-s".into(), "с".into());
        m.insert("text-duration-ms".into(), "мс".into());

        m.insert("text-duration-long-days-one".into(), "{ $n } день".into());
        m.insert("text-duration-long-days-few".into(), "{ $n } дні".into());
        m.insert("text-duration-long-days-many".into(), "{ $n } днів".into());
        m.insert("text-duration-long-days-other".into(), "{ $n } дня".into());

        m.insert(
            "text-duration-long-hours-one".into(),
            "{ $n } година".into(),
        );
        m.insert(
            "text-duration-long-hours-few".into(),
            "{ $n } години".into(),
        );
        m.insert(
            "text-duration-long-hours-many".into(),
            "{ $n } годин".into(),
        );
        m.insert(
            "text-duration-long-hours-other".into(),
            "{ $n } години".into(),
        );

        m.insert(
            "text-duration-long-minutes-one".into(),
            "{ $n } хвилина".into(),
        );
        m.insert(
            "text-duration-long-minutes-few".into(),
            "{ $n } хвилини".into(),
        );
        m.insert(
            "text-duration-long-minutes-many".into(),
            "{ $n } хвилин".into(),
        );
        m.insert(
            "text-duration-long-minutes-other".into(),
            "{ $n } хвилини".into(),
        );

        m.insert(
            "text-duration-long-seconds-one".into(),
            "{ $n } секунда".into(),
        );
        m.insert(
            "text-duration-long-seconds-few".into(),
            "{ $n } секунди".into(),
        );
        m.insert(
            "text-duration-long-seconds-many".into(),
            "{ $n } секунд".into(),
        );
        m.insert(
            "text-duration-long-seconds-other".into(),
            "{ $n } секунди".into(),
        );

        Self { messages: m }
    }
}

impl QuantityLabels for TestDurationLabels {
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
    fn decimal_separator(&self) -> String {
        ",".into()
    }
    fn duration_unit(&self, unit: &str) -> String {
        self.message(&format!("text-duration-{unit}"))
            .unwrap_or_else(|| unit.into())
    }
    fn locale(&self) -> String {
        "uk".into()
    }
    fn message(&self, id: &str) -> Option<String> {
        self.messages.get(id).cloned()
    }
    fn message_args(&self, id: &str, args: &[(&str, String)]) -> Option<String> {
        let pattern = self.messages.get(id)?;
        let mut out = pattern.clone();
        for (k, v) in args {
            out = out.replace(&format!("{{ ${k} }}"), v);
        }
        Some(out)
    }
}

fn render_duration(
    source: &str,
    val: Option<DataValue>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) -> Result<String, crate::template::TemplateError> {
    let compiled = CompiledTemplate::new(source)?;
    let inputs = vec![("d".to_string(), val, DataType::Any)];
    compiled.render_localized(
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
        RenderEnv {
            labels,
            now: Utc::now(),
            ..Default::default()
        },
        &crate::TypeRegistry::new(),
    )
}

#[test]
fn test_duration_short_default() {
    let canonical = Arc::new(crate::quantity::CanonicalLabels);
    let uk = Arc::new(TestDurationLabels::uk());

    // 1h 30m canonical
    let d = TimeDelta::hours(1) + TimeDelta::minutes(30);
    let res = render_duration(
        "{{ d | duration }}",
        Some(DataValue::Duration(d)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1h 30m");

    // short preset explicitly
    let res = render_duration(
        "{{ d | duration('short') }}",
        Some(DataValue::Duration(d)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1h 30m");

    // UK labels
    let res = render_duration("{{ d | duration }}", Some(DataValue::Duration(d)), uk).unwrap();
    assert_eq!(res, "1год 30хв");
}

#[test]
fn test_duration_long_plurals_and_zero_and_negative() {
    let canonical = Arc::new(crate::quantity::CanonicalLabels);
    let uk = Arc::new(TestDurationLabels::uk());

    // Zero
    let zero = TimeDelta::zero();
    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(zero)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "0 seconds");

    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(zero)),
        uk.clone(),
    )
    .unwrap();
    assert_eq!(res, "0 секунд");

    // 1h 1m 1s vs 2h 2m 2s vs 5h 5m 5s in UK
    let d1 = TimeDelta::hours(1) + TimeDelta::minutes(1) + TimeDelta::seconds(1);
    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(d1)),
        uk.clone(),
    )
    .unwrap();
    assert_eq!(res, "1 година 1 хвилина 1 секунда");

    let d2 = TimeDelta::hours(2) + TimeDelta::minutes(2) + TimeDelta::seconds(2);
    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(d2)),
        uk.clone(),
    )
    .unwrap();
    assert_eq!(res, "2 години 2 хвилини 2 секунди");

    let d5 = TimeDelta::hours(5) + TimeDelta::minutes(5) + TimeDelta::seconds(5);
    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(d5)),
        uk.clone(),
    )
    .unwrap();
    assert_eq!(res, "5 годин 5 хвилин 5 секунд");

    // Negative
    let neg = -(TimeDelta::minutes(1) + TimeDelta::seconds(30));
    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(neg)),
        canonical,
    )
    .unwrap();
    assert_eq!(res, "-1 minute 30 seconds");

    let res = render_duration(
        "{{ d | duration('long') }}",
        Some(DataValue::Duration(neg)),
        uk,
    )
    .unwrap();
    assert_eq!(res, "-1 хвилина 30 секунд");
}

#[test]
fn test_duration_pattern_rules() {
    let canonical = Arc::new(crate::quantity::CanonicalLabels);

    // Total hours when no d token: 26 hours 5 minutes 7 seconds
    let d =
        TimeDelta::days(1) + TimeDelta::hours(2) + TimeDelta::minutes(5) + TimeDelta::seconds(7);
    let res = render_duration(
        "{{ d | duration('H:mm:ss') }}",
        Some(DataValue::Duration(d)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "26:05:07");

    // Hours within day when d token is present
    let res = render_duration(
        "{{ d | duration('d [d] HH:mm:ss') }}",
        Some(DataValue::Duration(d)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1 d 02:05:07");

    // Total minutes when no d/H/HH token
    let res = render_duration(
        "{{ d | duration('m:ss') }}",
        Some(DataValue::Duration(d)),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1565:07");

    // Millis and negative
    let d_sub = -(TimeDelta::seconds(90) + TimeDelta::milliseconds(250));
    let res = render_duration(
        "{{ d | duration('m:ss.SSS') }}",
        Some(DataValue::Duration(d_sub)),
        canonical,
    )
    .unwrap();
    assert_eq!(res, "-1:30.250");
}

#[test]
fn test_duration_inputs_and_errors() {
    let canonical = Arc::new(crate::quantity::CanonicalLabels);

    // Number (seconds)
    let res = render_duration(
        "{{ d | duration('H:mm:ss') }}",
        Some(DataValue::Number(crate::NumberValue::Int(3665))),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1:01:05");

    // Canonical string text
    let res = render_duration(
        "{{ d | duration('short') }}",
        Some(DataValue::String("1h 30m".into())),
        canonical.clone(),
    )
    .unwrap();
    assert_eq!(res, "1h 30m");

    // Ticks input error
    let res = render_duration(
        "{{ d | duration }}",
        Some(DataValue::String("10t".into())),
        canonical,
    );
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.message.contains("duration needs a wall-clock duration"),
        "unexpected error message: {}",
        err.message
    );
}
