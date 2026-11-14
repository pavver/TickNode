use super::super::preview::{
    PinPreviewInputSource, PreviewStatus, collect_preview_inputs, evaluate_preview,
    is_sample_value_used, resolve_input_value,
};
use ticknode_shared::template::TemplateOptions;
use ticknode_shared::{DataType, DataValue, PinId};

#[test]
fn preview_input_collection_precedence() {
    let own = DataValue::from("from_own");
    let src = DataValue::from("from_source");
    let inline = DataValue::from("from_inline");
    let sample = DataValue::from("from_sample");

    // Own trace takes precedence over source trace, inline value, and sample value
    assert_eq!(
        resolve_input_value(Some(&own), Some(&src), Some(&inline), Some(&sample),),
        Some(own.clone())
    );

    // Source trace takes precedence over inline value and sample value when own trace is absent
    assert_eq!(
        resolve_input_value(None, Some(&src), Some(&inline), Some(&sample)),
        Some(src.clone())
    );

    // Inline value takes precedence over sample value when traces are absent
    assert_eq!(
        resolve_input_value(None, None, Some(&inline), Some(&sample)),
        Some(inline.clone())
    );

    // Sample value is used when traces and inline value are absent
    assert_eq!(
        resolve_input_value(None, None, None, Some(&sample)),
        Some(sample.clone())
    );

    // None when nothing has a value
    assert_eq!(resolve_input_value(None, None, None, None), None);
}

#[test]
fn preview_is_sample_value_used_truth_table() {
    let own = DataValue::from("from_own");
    let src = DataValue::from("from_source");
    let inline = DataValue::from("from_inline");
    let sample = DataValue::from("from_sample");

    assert!(!is_sample_value_used(Some(&own), None, None, Some(&sample)));
    assert!(!is_sample_value_used(None, Some(&src), None, Some(&sample)));
    assert!(!is_sample_value_used(
        None,
        None,
        Some(&inline),
        Some(&sample)
    ));
    assert!(is_sample_value_used(None, None, None, Some(&sample)));
    assert!(!is_sample_value_used(None, None, None, None));
}

#[test]
fn collect_preview_inputs_maps_pin_list() {
    let own = DataValue::from("v1");
    let src = DataValue::from(42i64);
    let sample = DataValue::from(true);

    let sources = [
        PinPreviewInputSource {
            pin_id: PinId(1),
            name: "first",
            declared_type: DataType::String,
            own_trace: Some(&own),
            source_trace: None,
            inline_value: None,
            sample_value: None,
        },
        PinPreviewInputSource {
            pin_id: PinId(2),
            name: "second",
            declared_type: DataType::Number,
            own_trace: None,
            source_trace: Some(&src),
            inline_value: None,
            sample_value: None,
        },
        PinPreviewInputSource {
            pin_id: PinId(3),
            name: "third",
            declared_type: DataType::Bool,
            own_trace: None,
            source_trace: None,
            inline_value: None,
            sample_value: Some(&sample),
        },
        PinPreviewInputSource {
            pin_id: PinId(4),
            name: "fourth",
            declared_type: DataType::Any,
            own_trace: None,
            source_trace: None,
            inline_value: None,
            sample_value: None,
        },
    ];

    let result = collect_preview_inputs(&sources);
    assert_eq!(result.len(), 4);
    assert_eq!(
        result[0],
        ("first".to_string(), Some(own), DataType::String)
    );
    assert_eq!(
        result[1],
        ("second".to_string(), Some(src), DataType::Number)
    );
    assert_eq!(
        result[2],
        ("third".to_string(), Some(sample), DataType::Bool)
    );
    assert_eq!(result[3], ("fourth".to_string(), None, DataType::Any));
}

#[test]
fn evaluate_preview_syntax_error_has_priority() {
    let status = evaluate_preview("{{ 1 + }}", &[], TemplateOptions::default());
    assert!(matches!(status, PreviewStatus::SyntaxError { line: 1, .. }));
}

#[test]
fn evaluate_preview_waiting_when_all_inputs_have_no_value() {
    let inputs = vec![
        ("a".to_string(), None, DataType::Number),
        ("b".to_string(), None, DataType::String),
    ];
    let status = evaluate_preview("Hello {{ a }}", &inputs, TemplateOptions::default());
    assert_eq!(status, PreviewStatus::Waiting);
}

#[test]
fn evaluate_preview_empty_rendered_text() {
    let status = evaluate_preview(
        "{% if false %}something{% endif %}",
        &[],
        TemplateOptions::default(),
    );
    assert_eq!(status, PreviewStatus::Empty);
}

#[test]
fn evaluate_preview_renders_valid_template() {
    let inputs = vec![(
        "name".to_string(),
        Some(DataValue::from("Alice")),
        DataType::String,
    )];
    let status = evaluate_preview("Hello {{ name }}!", &inputs, TemplateOptions::default());
    assert_eq!(status, PreviewStatus::Text("Hello Alice!".to_string()));
}

#[test]
fn evaluate_preview_waits_when_a_missing_input_breaks_rendering() {
    let inputs = vec![
        (
            "value".to_string(),
            Some(DataValue::from("world")),
            DataType::String,
        ),
        ("msg".to_string(), None, DataType::Any),
    ];
    assert_eq!(
        evaluate_preview(
            "{{ value }} {{ msg.chat.id }}",
            &inputs,
            TemplateOptions::default()
        ),
        PreviewStatus::Waiting
    );
    let complete = vec![(
        "value".to_string(),
        Some(DataValue::from("world")),
        DataType::String,
    )];
    assert!(matches!(
        evaluate_preview("{{ value.a.b }}", &complete, TemplateOptions::default()),
        PreviewStatus::RenderError(_)
    ));
}

#[test]
fn evaluate_preview_supports_template_options_trim_blocks() {
    let source = "Status:\n{% if on %}\n  Light is on\n{% endif %}\n";
    let inputs = vec![(
        "on".to_string(),
        Some(DataValue::Bool(true)),
        DataType::Bool,
    )];

    let untrimmed = evaluate_preview(source, &inputs, TemplateOptions { trim_blocks: false });
    assert_eq!(
        untrimmed,
        PreviewStatus::Text("Status:\n\n  Light is on\n".to_string())
    );

    let trimmed = evaluate_preview(source, &inputs, TemplateOptions { trim_blocks: true });
    assert_eq!(
        trimmed,
        PreviewStatus::Text("Status:\n  Light is on\n".to_string())
    );
}

#[test]
fn evaluate_preview_with_sample_inputs() {
    let sources = [
        PinPreviewInputSource {
            pin_id: PinId(1),
            name: "greeting",
            declared_type: DataType::String,
            own_trace: None,
            source_trace: None,
            inline_value: None,
            sample_value: Some(&DataValue::from("Hello")),
        },
        PinPreviewInputSource {
            pin_id: PinId(2),
            name: "user",
            declared_type: DataType::String,
            own_trace: None,
            source_trace: None,
            inline_value: None,
            sample_value: Some(&DataValue::from("Alice")),
        },
    ];
    let inputs = collect_preview_inputs(&sources);
    assert_eq!(
        evaluate_preview(
            "{{ greeting }}, {{ user }}!",
            &inputs,
            TemplateOptions::default()
        ),
        PreviewStatus::Text("Hello, Alice!".to_string())
    );
}
