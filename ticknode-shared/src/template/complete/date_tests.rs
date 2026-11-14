use super::*;
use crate::types::{FieldDefinition, TypeDefinition};

fn parse_caret(template: &str) -> (String, usize) {
    let caret_marker = "|^|";
    let pos = template
        .find(caret_marker)
        .expect("test template must contain caret marker |^|");
    let source = template.replace(caret_marker, "");
    (source, pos)
}

fn complete_at<'a>(
    template: &'a str,
    inputs: &'a [(String, DataType)],
    types: &'a TypeRegistry,
) -> (String, Option<Completion>) {
    complete_at_full(template, inputs, types, None, None)
}

fn complete_at_full<'a>(
    template: &'a str,
    inputs: &'a [(String, DataType)],
    types: &'a TypeRegistry,
    now: Option<chrono::DateTime<chrono::Local>>,
    labels: Option<&'a dyn crate::quantity::QuantityLabels>,
) -> (String, Option<Completion>) {
    let (source, caret) = parse_caret(template);
    let input = CompletionInput {
        source: &source,
        caret,
        inputs,
        types,
        now,
        labels,
    };
    let completion = complete(&input);
    (source, completion)
}

fn setup_test_context() -> (Vec<(String, DataType)>, TypeRegistry) {
    let state_struct = TypeDefinition::new_struct(
        "test.entity_state",
        "EntityState",
        vec![FieldDefinition::new(
            "last_changed",
            "Last Changed",
            DataType::DateTime,
        )],
    );
    let types = TypeRegistry::from_definitions(vec![state_struct]);

    let inputs = vec![
        (
            "value2".to_string(),
            DataType::Named("test.entity_state".to_string()),
        ),
        ("when".to_string(), DataType::DateTime),
        ("span".to_string(), DataType::Duration),
        ("clock".to_string(), DataType::Time),
    ];

    (inputs, types)
}

#[test]
fn test_datetime_struct_field_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ value2.last_changed | |^|}}", &inputs, &types);
    let c = res.expect("must return completion for value2.last_changed filter");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();

    for expected in ["date", "strftime", "text", "default"] {
        assert!(
            labels.contains(&expected),
            "value2.last_changed must offer filter '{expected}', got: {labels:?}"
        );
    }

    for forbidden in ["upper", "capitalize", "length", "split"] {
        assert!(
            !labels.contains(&forbidden),
            "value2.last_changed must not offer string filter '{forbidden}'"
        );
    }
}

#[test]
fn test_plain_datetime_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ when | |^|}}", &inputs, &types);
    let c = res.expect("must return completion for when filter");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();

    for expected in ["date", "strftime", "text", "default"] {
        assert!(
            labels.contains(&expected),
            "when must offer filter '{expected}', got: {labels:?}"
        );
    }

    for forbidden in ["upper", "capitalize", "length", "split"] {
        assert!(
            !labels.contains(&forbidden),
            "when must not offer string filter '{forbidden}'"
        );
    }
}

#[test]
fn test_duration_filter_completion() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ span | |^|}}", &inputs, &types);
    let c = res.expect("must return completion for span filter");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();

    for expected in ["text", "default"] {
        assert!(
            labels.contains(&expected),
            "span must offer filter '{expected}', got: {labels:?}"
        );
    }

    for forbidden in ["date", "strftime", "upper", "capitalize", "length", "split"] {
        assert!(
            !labels.contains(&forbidden),
            "span must not offer filter '{forbidden}'"
        );
    }
}

#[test]
fn test_chained_date_filter_infers_string() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ when | date | |^|}}", &inputs, &types);
    let c = res.expect("must return completion after chained date filter");
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels.contains(&"upper"),
        "chained date filter must infer String and offer 'upper', got: {labels:?}"
    );
    assert!(labels.contains(&"capitalize"));
    assert!(labels.contains(&"text"));
    assert!(labels.contains(&"default"));
}

#[test]
fn test_datetime_and_duration_tests_completion() {
    let (inputs, types) = setup_test_context();

    // DateTime accepts eq, ne, lt, le, gt, ge, defined, etc.
    let (_, res_dt) = complete_at("{{ when is |^|}}", &inputs, &types);
    let c_dt = res_dt.expect("completion for when is");
    let labels_dt: Vec<_> = c_dt.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["eq", "ne", "lt", "le", "gt", "ge", "defined"] {
        assert!(
            labels_dt.contains(&expected),
            "when is must offer test '{expected}'"
        );
    }
    for forbidden in ["odd", "even", "lower", "upper"] {
        assert!(
            !labels_dt.contains(&forbidden),
            "when is must not offer test '{forbidden}'"
        );
    }

    // Duration accepts eq, ne, defined, but NOT lt, le, gt, ge
    let (_, res_dur) = complete_at("{{ span is |^|}}", &inputs, &types);
    let c_dur = res_dur.expect("completion for span is");
    let labels_dur: Vec<_> = c_dur.items.iter().map(|i| i.label.as_str()).collect();
    for expected in ["eq", "ne", "defined"] {
        assert!(
            labels_dur.contains(&expected),
            "span is must offer test '{expected}'"
        );
    }
    for forbidden in ["lt", "le", "gt", "ge", "odd", "even", "lower", "upper"] {
        assert!(
            !labels_dur.contains(&forbidden),
            "span is must not offer test '{forbidden}'"
        );
    }
}

#[test]
fn test_filter_help_messages() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ when | |^|}}", &inputs, &types);
    let c = res.expect("must return completion for when filter");

    let date_item = c
        .items
        .iter()
        .find(|i| i.label == "date")
        .expect("date item");
    assert_eq!(
        date_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-filter-date")
    );

    let strftime_item = c
        .items
        .iter()
        .find(|i| i.label == "strftime")
        .expect("strftime item");
    assert_eq!(
        strftime_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-filter-strftime")
    );

    let text_item = c
        .items
        .iter()
        .find(|i| i.label == "text")
        .expect("text item");
    assert_eq!(
        text_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-filter-text")
    );

    let default_item = c
        .items
        .iter()
        .find(|i| i.label == "default")
        .expect("default item");
    assert_eq!(
        default_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-filter-default")
    );
}

#[test]
fn test_date_pattern_context_prefix_m() {
    let (inputs, types) = setup_test_context();
    let (source, res) = complete_at("{{ when | date('DD.M|^|') }}", &inputs, &types);
    let c = res.expect("must return completion inside date pattern");

    // Only tokens starting with 'M' (case-sensitive)
    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(labels, vec!["MMMM", "MMM", "MM", "M"]);

    // Range should replace 'M'
    let m_pos = source.find(".M'").expect("M pos") + 1;
    assert_eq!(c.range, m_pos..m_pos + 1);

    for item in &c.items {
        assert_eq!(item.kind, CompletionKind::Format);
        assert!(item.help.is_some());
    }

    let mm_item = c.items.iter().find(|i| i.label == "MM").expect("MM item");
    assert_eq!(
        mm_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-date-token-month2")
    );
}

#[test]
fn test_date_pattern_empty_string_presets_first() {
    let (inputs, types) = setup_test_context();
    let (_, res) = complete_at("{{ when | date('|^|') }}", &inputs, &types);
    let c = res.expect("must return completion for empty date pattern");

    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.len() > 5);

    // Presets first
    assert_eq!(&labels[0..5], &["datetime", "date", "time", "long", "iso"]);

    // Followed by date tokens
    assert!(labels.contains(&"YYYY"));
    assert!(labels.contains(&"MMMM"));
    assert!(labels.contains(&"[text]"));

    let iso_item = c.items.iter().find(|i| i.label == "iso").expect("iso item");
    assert_eq!(
        iso_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-date-preset-iso")
    );

    let lit_item = c
        .items
        .iter()
        .find(|i| i.label == "[text]")
        .expect("[text] item");
    assert_eq!(lit_item.insert, "[]");
    assert_eq!(
        lit_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-date-literal")
    );
}

#[test]
fn test_strftime_pattern_completion() {
    let (inputs, types) = setup_test_context();
    let (source, res) = complete_at("{{ when | strftime('%d.%|^|') }}", &inputs, &types);
    let c = res.expect("must return completion inside strftime");

    let labels: Vec<_> = c.items.iter().map(|i| i.label.as_str()).collect();
    for expected in [
        "%Y", "%y", "%m", "%B", "%b", "%d", "%e", "%A", "%a", "%j", "%H", "%I", "%M", "%S", "%f",
        "%p", "%z", "%Z", "%%",
    ] {
        assert!(
            labels.contains(&expected),
            "strftime must contain {expected}"
        );
    }

    let percent_pos = source.rfind('%').expect("second % pos");
    assert_eq!(c.range, percent_pos..percent_pos + 1);

    let y_item = c.items.iter().find(|i| i.label == "%Y").expect("%Y item");
    assert_eq!(y_item.kind, CompletionKind::Format);
    assert_eq!(
        y_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-strftime-upper-y")
    );

    let pct_item = c.items.iter().find(|i| i.label == "%%").expect("%% item");
    assert_eq!(
        pct_item.help.as_ref().map(|m| m.key.as_str()),
        Some("template-help-strftime-percent")
    );
}

#[test]
fn test_unrelated_string_literal_returns_none() {
    let (inputs, types) = setup_test_context();
    let (_, res1) = complete_at("{{ \"hello |^| world\" }}", &inputs, &types);
    assert!(res1.is_none());

    let (_, res2) = complete_at("{{ when | upper('|^|') }}", &inputs, &types);
    assert!(res2.is_none());

    let (_, res3) = complete_at("{{ when | date(123, '|^|') }}", &inputs, &types);
    assert!(res3.is_none());
}

#[test]
fn test_date_and_strftime_examples_with_fixed_time() {
    use crate::quantity::CanonicalLabels;
    use chrono::{DateTime, Datelike, Local};

    let (inputs, types) = setup_test_context();
    let fixed_time = DateTime::from_timestamp(1_700_000_000, 0)
        .expect("valid timestamp")
        .with_timezone(&Local);
    let labels = CanonicalLabels;

    let (_, res_date) = complete_at_full(
        "{{ when | date('|^|') }}",
        &inputs,
        &types,
        Some(fixed_time),
        Some(&labels),
    );
    let c_date = res_date.expect("completion with time");

    let yyyy_item = c_date
        .items
        .iter()
        .find(|i| i.label == "YYYY")
        .expect("YYYY item");
    assert_eq!(yyyy_item.detail, format!("{:04}", fixed_time.year()));

    let datetime_item = c_date
        .items
        .iter()
        .find(|i| i.label == "datetime")
        .expect("datetime item");
    assert!(!datetime_item.detail.is_empty());

    let (_, res_strftime) = complete_at_full(
        "{{ when | strftime('%|^|') }}",
        &inputs,
        &types,
        Some(fixed_time),
        Some(&labels),
    );
    let c_strftime = res_strftime.expect("completion for strftime");

    let y_item = c_strftime
        .items
        .iter()
        .find(|i| i.label == "%Y")
        .expect("%Y item");
    assert_eq!(y_item.detail, format!("{}", fixed_time.format("%Y")));
}
