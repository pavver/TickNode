//! Pure unit tests for template field helpers (no browser required).

use super::caret::{apply_completion, byte_to_utf16, utf16_to_byte};
use super::preview::{collect_preview_inputs, evaluate_preview, resolve_input_value, PinPreviewInputSource, PreviewStatus};
use ticknode_shared::{DataType, DataValue, PinId};

#[test]
fn utf16_and_byte_conversions_ascii() {
    let s = "Hello, world!";
    assert_eq!(utf16_to_byte(s, 0), 0);
    assert_eq!(byte_to_utf16(s, 0), 0);
    assert_eq!(utf16_to_byte(s, 7), 7);
    assert_eq!(byte_to_utf16(s, 7), 7);
    assert_eq!(utf16_to_byte(s, 100), s.len());
    assert_eq!(byte_to_utf16(s, 100), s.encode_utf16().count());
}

#[test]
fn utf16_and_byte_conversions_cyrillic() {
    let s = "Привіт, світ!";
    // Each Cyrillic letter is 2 bytes in UTF-8 and 1 code unit in UTF-16
    // "П" is 2 bytes (0..2), 1 u16 (0)
    // "р" is 2 bytes (2..4), 1 u16 (1)
    // "и" is 2 bytes (4..6), 1 u16 (2)
    assert_eq!(utf16_to_byte(s, 0), 0);
    assert_eq!(byte_to_utf16(s, 0), 0);
    assert_eq!(utf16_to_byte(s, 1), 2);
    assert_eq!(byte_to_utf16(s, 2), 1);
    assert_eq!(utf16_to_byte(s, 3), 6);
    assert_eq!(byte_to_utf16(s, 6), 3);
    assert_eq!(utf16_to_byte(s, 100), s.len());
    assert_eq!(byte_to_utf16(s, 100), s.encode_utf16().count());
}

#[test]
fn utf16_and_byte_conversions_emoji_surrogate_pairs() {
    // '👋' (U+1F44B) is 4 bytes in UTF-8 and 2 code units in UTF-16 (surrogate pair)
    // '🌍' (U+1F30D) is 4 bytes in UTF-8 and 2 code units in UTF-16
    let s = "👋🌍!";
    assert_eq!(utf16_to_byte(s, 0), 0);
    assert_eq!(byte_to_utf16(s, 0), 0);
    // After first emoji (2 u16 code units), byte offset is 4
    assert_eq!(utf16_to_byte(s, 2), 4);
    assert_eq!(byte_to_utf16(s, 4), 2);
    // Inside first surrogate pair: clamps to char boundary
    assert_eq!(utf16_to_byte(s, 1), 0);
    // After second emoji (4 u16 code units), byte offset is 8
    assert_eq!(utf16_to_byte(s, 4), 8);
    assert_eq!(byte_to_utf16(s, 8), 4);
    // '!' is 1 byte, 1 u16 code unit
    assert_eq!(utf16_to_byte(s, 5), 9);
    assert_eq!(byte_to_utf16(s, 9), 5);
}

#[test]
fn apply_completion_variable_and_field() {
    let source = "Hello {{ va }}";
    // Splicing over "va" (range 9..11)
    let (res, caret) = apply_completion(source, 9..11, "value");
    assert_eq!(res, "Hello {{ value }}");
    assert_eq!(caret, 14); // right after "value"
}

#[test]
fn apply_completion_function_places_caret_between_parentheses() {
    let source = "{{ lip }}";
    // Splicing "lip" with "lipsum()"
    let (res, caret) = apply_completion(source, 3..6, "lipsum()");
    assert_eq!(res, "{{ lipsum() }}");
    // "lipsum()" length is 8; caret should be at 3 + 8 - 1 = 10 (inside ())
    assert_eq!(caret, 10);
    assert_eq!(&res[9..11], "()");
}

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
    assert_eq!(result[0], ("first".to_string(), Some(own), DataType::String));
    assert_eq!(result[1], ("second".to_string(), Some(src), DataType::Number));
    assert_eq!(result[2], ("third".to_string(), None, DataType::Bool));
}

#[test]
fn evaluate_preview_syntax_error_has_priority() {
    let status = evaluate_preview("{{ 1 + }}", &[]);
    assert!(matches!(status, PreviewStatus::SyntaxError { line: 1, .. }));
}

#[test]
fn evaluate_preview_waiting_when_all_inputs_have_no_value() {
    let inputs = vec![
        ("a".to_string(), None, DataType::Number),
        ("b".to_string(), None, DataType::String),
    ];
    let status = evaluate_preview("Hello {{ a }}", &inputs);
    assert_eq!(status, PreviewStatus::Waiting);
}

#[test]
fn evaluate_preview_empty_rendered_text() {
    let status = evaluate_preview("{% if false %}something{% endif %}", &[]);
    assert_eq!(status, PreviewStatus::Empty);
}

#[test]
fn evaluate_preview_renders_valid_template() {
    let inputs = vec![("name".to_string(), Some(DataValue::from("World")), DataType::String)];
    let status = evaluate_preview("Hello {{ name }}!", &inputs);
    assert_eq!(status, PreviewStatus::Text("Hello World!".to_string()));
}

#[test]
fn trigger_chars_identify_completion_positions() {
    use super::completion::is_completion_trigger_char;

    assert!(is_completion_trigger_char("{{ v", false));
    assert!(is_completion_trigger_char("{{ a.", false));
    assert!(is_completion_trigger_char("{{ a|", false));
    assert!(is_completion_trigger_char("{{ a is ", false));
    assert!(is_completion_trigger_char("{% ", false));
    assert!(is_completion_trigger_char("{%- ", false));
    assert!(!is_completion_trigger_char("hello world ", false));
    assert!(is_completion_trigger_char("hello", true));
}
