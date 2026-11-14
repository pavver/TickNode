use crate::quantity::QuantityLabels;
use crate::template::{CompiledTemplate, RenderEnv};
use crate::{DataType, DataValue};
use chrono::{DateTime, TimeDelta, Utc};
use std::collections::HashMap;
use std::sync::Arc;

struct UkTestLabels {
    messages: HashMap<String, String>,
}

impl UkTestLabels {
    fn new() -> Self {
        let mut m = HashMap::new();
        m.insert("text-ago-now".into(), "щойно".into());

        for (unit, one, few, many, other) in [
            (
                "seconds",
                "{ $n } секунду тому",
                "{ $n } секунди тому",
                "{ $n } секунд тому",
                "{ $n } секунди тому",
            ),
            (
                "minutes",
                "{ $n } хвилину тому",
                "{ $n } хвилини тому",
                "{ $n } хвилин тому",
                "{ $n } хвилини тому",
            ),
            (
                "hours",
                "{ $n } годину тому",
                "{ $n } години тому",
                "{ $n } годин тому",
                "{ $n } години тому",
            ),
            (
                "days",
                "{ $n } день тому",
                "{ $n } дні тому",
                "{ $n } днів тому",
                "{ $n } дня тому",
            ),
            (
                "months",
                "{ $n } місяць тому",
                "{ $n } місяці тому",
                "{ $n } місяців тому",
                "{ $n } місяця тому",
            ),
            (
                "years",
                "{ $n } рік тому",
                "{ $n } роки тому",
                "{ $n } років тому",
                "{ $n } року тому",
            ),
        ] {
            m.insert(format!("text-ago-past-{unit}-one"), one.into());
            m.insert(format!("text-ago-past-{unit}-few"), few.into());
            m.insert(format!("text-ago-past-{unit}-many"), many.into());
            m.insert(format!("text-ago-past-{unit}-other"), other.into());
        }

        for (unit, one, few, many, other) in [
            (
                "seconds",
                "через { $n } секунду",
                "через { $n } секунди",
                "через { $n } секунд",
                "через { $n } секунди",
            ),
            (
                "minutes",
                "через { $n } хвилину",
                "через { $n } хвилини",
                "через { $n } хвилин",
                "через { $n } хвилини",
            ),
            (
                "hours",
                "через { $n } годину",
                "через { $n } години",
                "через { $n } годин",
                "через { $n } години",
            ),
            (
                "days",
                "через { $n } день",
                "через { $n } дні",
                "через { $n } днів",
                "через { $n } дня",
            ),
            (
                "months",
                "через { $n } місяць",
                "через { $n } місяці",
                "через { $n } місяців",
                "через { $n } місяця",
            ),
            (
                "years",
                "через { $n } рік",
                "через { $n } роки",
                "через { $n } років",
                "через { $n } року",
            ),
        ] {
            m.insert(format!("text-ago-future-{unit}-one"), one.into());
            m.insert(format!("text-ago-future-{unit}-few"), few.into());
            m.insert(format!("text-ago-future-{unit}-many"), many.into());
            m.insert(format!("text-ago-future-{unit}-other"), other.into());
        }

        Self { messages: m }
    }
}

impl QuantityLabels for UkTestLabels {
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
    fn decimal_separator(&self) -> String {
        ",".into()
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

fn render_ago(
    source: &str,
    val: Option<DataValue>,
    now: DateTime<Utc>,
    labels: Arc<dyn QuantityLabels + Send + Sync>,
) -> Result<String, crate::template::TemplateError> {
    let compiled = CompiledTemplate::new(source)?;
    let inputs = vec![("t".to_string(), val, DataType::Any)];
    compiled.render_localized(
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
        RenderEnv { labels, now },
    )
}

#[test]
fn test_ago_just_now() {
    let now = DateTime::parse_from_rfc3339("2026-11-14T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let labels = Arc::new(UkTestLabels::new());

    // Past 10s
    let t = now - TimeDelta::seconds(10);
    let res = render_ago(
        "{{ t | ago }}",
        Some(DataValue::DateTime(t)),
        now,
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "щойно");

    // Future 20s
    let t = now + TimeDelta::seconds(20);
    let res = render_ago(
        "{{ t | ago }}",
        Some(DataValue::DateTime(t)),
        now,
        labels.clone(),
    )
    .unwrap();
    assert_eq!(res, "щойно");
}

#[test]
fn test_ago_past_and_future_every_unit_uk_plurals() {
    let now = DateTime::parse_from_rfc3339("2026-11-14T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let labels = Arc::new(UkTestLabels::new());

    // Minutes: 1 min, 2 min, 5 min
    let cases = [
        // Past
        (TimeDelta::minutes(1), "{{ t | ago }}", "1 хвилину тому"),
        (TimeDelta::minutes(2), "{{ t | ago }}", "2 хвилини тому"),
        (TimeDelta::minutes(5), "{{ t | ago }}", "5 хвилин тому"),
        // Hours
        (TimeDelta::hours(1), "{{ t | ago }}", "1 годину тому"),
        (TimeDelta::hours(2), "{{ t | ago }}", "2 години тому"),
        (TimeDelta::hours(5), "{{ t | ago }}", "5 годин тому"),
        // Days
        (TimeDelta::days(1), "{{ t | ago }}", "1 день тому"),
        (TimeDelta::days(2), "{{ t | ago }}", "2 дні тому"),
        (TimeDelta::days(5), "{{ t | ago }}", "5 днів тому"),
        // Months
        (TimeDelta::days(31), "{{ t | ago }}", "1 місяць тому"),
        (TimeDelta::days(61), "{{ t | ago }}", "2 місяці тому"),
        (TimeDelta::days(153), "{{ t | ago }}", "5 місяців тому"),
        // Years
        (TimeDelta::days(366), "{{ t | ago }}", "1 рік тому"),
        (TimeDelta::days(732), "{{ t | ago }}", "2 роки тому"),
        (TimeDelta::days(1830), "{{ t | ago }}", "5 років тому"),
    ];

    for (delta, tmpl, expected) in cases {
        let t = now - delta;
        let res = render_ago(tmpl, Some(DataValue::DateTime(t)), now, labels.clone()).unwrap();
        assert_eq!(res, expected, "failed for delta past {delta:?}");
    }

    // Future
    let future_cases = [
        (TimeDelta::minutes(1), "{{ t | ago }}", "через 1 хвилину"),
        (TimeDelta::minutes(2), "{{ t | ago }}", "через 2 хвилини"),
        (TimeDelta::minutes(5), "{{ t | ago }}", "через 5 хвилин"),
        (TimeDelta::hours(1), "{{ t | ago }}", "через 1 годину"),
        (TimeDelta::hours(2), "{{ t | ago }}", "через 2 години"),
        (TimeDelta::hours(5), "{{ t | ago }}", "через 5 годин"),
        (TimeDelta::days(1), "{{ t | ago }}", "через 1 день"),
        (TimeDelta::days(2), "{{ t | ago }}", "через 2 дні"),
        (TimeDelta::days(5), "{{ t | ago }}", "через 5 днів"),
        (TimeDelta::days(31), "{{ t | ago }}", "через 1 місяць"),
        (TimeDelta::days(61), "{{ t | ago }}", "через 2 місяці"),
        (TimeDelta::days(153), "{{ t | ago }}", "через 5 місяців"),
        (TimeDelta::days(366), "{{ t | ago }}", "через 1 рік"),
        (TimeDelta::days(732), "{{ t | ago }}", "через 2 роки"),
        (TimeDelta::days(1830), "{{ t | ago }}", "через 5 років"),
    ];

    for (delta, tmpl, expected) in future_cases {
        let t = now + delta;
        let res = render_ago(tmpl, Some(DataValue::DateTime(t)), now, labels.clone()).unwrap();
        assert_eq!(res, expected, "failed for delta future {delta:?}");
    }
}

#[test]
fn test_ago_unix_input() {
    let now = DateTime::parse_from_rfc3339("2026-11-14T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let labels = Arc::new(UkTestLabels::new());
    let past = now - TimeDelta::minutes(5);
    let unix = past.timestamp();

    let res = render_ago(
        "{{ t | ago }}",
        Some(DataValue::Number(crate::NumberValue::Int(unix))),
        now,
        labels,
    )
    .unwrap();
    assert_eq!(res, "5 хвилин тому");
}

#[test]
fn test_ago_time_input_error() {
    let now = Utc::now();
    let labels = Arc::new(UkTestLabels::new());
    let time = chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap();

    let res = render_ago("{{ t | ago }}", Some(DataValue::Time(time)), now, labels);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.message.contains("ago needs a date-time, not a time"),
        "unexpected error message: {}",
        err.message
    );
}

#[test]
fn test_now_injected_instant_and_variables() {
    let now = DateTime::parse_from_rfc3339("2026-11-14T15:30:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let compiled = CompiledTemplate::new("{{ now() }}").unwrap();
    let res = compiled
        .render_localized(
            &[],
            &|v, _| crate::text::format_value(v),
            Arc::new(crate::text::format_value),
            RenderEnv {
                labels: Arc::new(crate::quantity::CanonicalLabels),
                now,
            },
        )
        .unwrap();
    assert_eq!(res, "2026-11-14T15:30:00+00:00");

    let vars = crate::template::variables("{{ now() }}").unwrap();
    assert!(vars.is_empty(), "now() should not be derived as variable");
}
