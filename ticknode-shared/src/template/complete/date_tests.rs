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
    let (source, caret) = parse_caret(template);
    let input = CompletionInput {
        source: &source,
        caret,
        inputs,
        types,
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
