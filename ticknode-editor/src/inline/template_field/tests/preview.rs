use super::super::preview::{
    PinPreviewInputSource, PreviewStatus, collect_preview_inputs, evaluate_preview,
    resolve_input_value,
};
use ticknode_shared::template::TemplateOptions;
use ticknode_shared::{DataType, DataValue, PinId};

#[test]
fn preview_input_collection_precedence() {
    let own = DataValue::from("from_own");
    let src = DataValue::from("from_source");
    let inline = DataValue::from("from_inline");

    // Own trace takes precedence over source trace and inline value
    assert_eq!(
        resolve_input_value(Some(&own), Some(&src), Some(&inline)),
        Some(own.clone())
    );

    // Source trace takes precedence over inline value when own trace is absent
    assert_eq!(
        resolve_input_value(None, Some(&src), Some(&inline)),
        Some(src.clone())
    );

    // Inline value is used when traces are absent
    assert_eq!(
        resolve_input_value(None, None, Some(&inline)),
        Some(inline.clone())
    );

    // None when nothing has a value
    assert_eq!(resolve_input_value(None, None, None), None);
}

#[test]
fn collect_preview_inputs_maps_pin_list() {
    let own = DataValue::from("v1");
    let src = DataValue::from(42i64);

    let sources = [
        PinPreviewInputSource {
            pin_id: PinId(1),
            name: "first",
            declared_type: DataType::String,
            own_trace: Some(&own),
            source_trace: None,
            inline_value: None,
        },
        PinPreviewInputSource {
            pin_id: PinId(2),
            name: "second",
            declared_type: DataType::Number,
            own_trace: None,
            source_trace: Some(&src),
            inline_value: None,
        },
        PinPreviewInputSource {
            pin_id: PinId(3),
            name: "third",
            declared_type: DataType::Bool,
            own_trace: None,
            source_trace: None,
            inline_value: None,
        },
    ];

    let result = collect_preview_inputs(&sources);
    assert_eq!(result.len(), 3);
    assert_eq!(
        result[0],
        ("first".to_string(), Some(own), DataType::String)
    );
    assert_eq!(
        result[1],
        ("second".to_string(), Some(src), DataType::Number)
    );
    assert_eq!(result[2], ("third".to_string(), None, DataType::Bool));
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
