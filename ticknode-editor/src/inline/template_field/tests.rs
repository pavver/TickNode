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

#[test]
fn hover_token_at_plain_text_returns_none() {
    use super::hover::token_at;
    let source = "Hello world";
    for i in 0..source.len() {
        assert_eq!(token_at(source, i), None);
    }
}

#[test]
fn hover_token_at_single_variable_over_each_char() {
    use super::hover::{token_at, HoverToken};
    let source = "{{ value }}";
    let val_start = source.find("value").unwrap();
    let val_end = val_start + "value".len();
    for i in val_start..val_end {
        assert_eq!(
            token_at(source, i),
            Some(HoverToken {
                range: val_start..val_end,
                path: vec!["value".to_string()],
            })
        );
    }
}

#[test]
fn hover_token_at_dotted_path_over_u_and_name() {
    use super::hover::{token_at, HoverToken};
    let source = "{{ u.name }}";
    let u_pos = source.find('u').unwrap();
    assert_eq!(
        token_at(source, u_pos),
        Some(HoverToken {
            range: u_pos..u_pos + 1,
            path: vec!["u".to_string()],
        })
    );

    let name_start = source.find("name").unwrap();
    let name_end = name_start + "name".len();
    for i in name_start..name_end {
        assert_eq!(
            token_at(source, i),
            Some(HoverToken {
                range: name_start..name_end,
                path: vec!["u".to_string(), "name".to_string()],
            })
        );
    }
}

#[test]
fn hover_token_at_cyrillic_text_before_expression() {
    use super::hover::{token_at, HoverToken};
    let source = "Привіт, світ! {{ val }}";
    let val_pos = source.find("val").unwrap();
    // Over Cyrillic text
    for (i, _) in source[..val_pos - 3].char_indices() {
        assert_eq!(token_at(source, i), None);
    }
    // Over val
    assert_eq!(
        token_at(source, val_pos),
        Some(HoverToken {
            range: val_pos..val_pos + 3,
            path: vec!["val".to_string()],
        })
    );
}

#[test]
fn hover_token_at_string_literal_returns_none() {
    use super::hover::token_at;
    let source = "{{ 'value' }}";
    let val_pos = source.find("value").unwrap();
    for i in val_pos..val_pos + "value".len() {
        assert_eq!(token_at(source, i), None);
    }
}

#[test]
fn hover_token_at_text_prefix_gives_two_segments() {
    use super::hover::{token_at, HoverToken};
    let source = "{{ text.x }}";
    let x_pos = source.rfind('x').unwrap();
    assert_eq!(
        token_at(source, x_pos),
        Some(HoverToken {
            range: x_pos..x_pos + 1,
            path: vec!["text".to_string(), "x".to_string()],
        })
    );
}

#[test]
fn hover_find_expression_tokens() {
    use super::hover::find_expression_tokens;
    let source = "Hi {{ u.name }} and {% if active %} {# skip #} {{ 'str' }}";
    let tokens = find_expression_tokens(source);
    let token_texts: Vec<&str> = tokens.iter().map(|r| &source[r.clone()]).collect();
    assert_eq!(token_texts, vec!["u", "name", "if", "active"]);
}

#[test]
fn hover_field_resolution_and_formatting() {
    use super::hover::{format_field_tooltip, resolve_field_path_type, resolve_field_path_value};
    use ticknode_shared::types::{FieldDefinition, TypeDefinition, TypeKind};
    use ticknode_shared::TypeRegistry;

    let struct_type = TypeDefinition {
        name: "test.user".to_string(),
        title: "User".to_string(),
        description: "User struct".to_string(),
        color: Some("#ffffff".to_string()),
        kind: TypeKind::Struct {
            fields: vec![
                FieldDefinition {
                    name: "name".to_string(),
                    title: "Name".to_string(),
                    description: "User name".to_string(),
                    data_type: DataType::String,
                    optional: false,
                },
                FieldDefinition {
                    name: "age".to_string(),
                    title: "Age".to_string(),
                    description: "User age".to_string(),
                    data_type: DataType::Number,
                    optional: false,
                },
            ],
        },
        plugin: None,
    };
    let types = TypeRegistry::from_definitions(vec![struct_type]);

    let root_type = DataType::Named("test.user".to_string());
    let path = vec!["u".to_string(), "name".to_string()];
    let field_type = resolve_field_path_type(&root_type, &path[1..], &types);
    assert_eq!(field_type, Some(DataType::String));

    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "name".to_string(),
        ticknode_shared::ObjectField {
            name: "name".into(),
            description: String::new(),
            value: DataValue::from("Alice"),
        },
    );
    fields.insert(
        "age".to_string(),
        ticknode_shared::ObjectField {
            name: "age".into(),
            description: String::new(),
            value: DataValue::from(30),
        },
    );
    let obj = DataValue::Object {
        fields,
        type_name: Some("test.user".to_string()),
    };

    let val = resolve_field_path_value(Some(&obj), &path[1..]);
    assert_eq!(val, Some(DataValue::from("Alice")));

    let tooltip = format_field_tooltip(&path, &DataType::String, val.as_ref(), "String");
    assert_eq!(tooltip, "u.name: String\nAlice");

    let missing_tooltip = format_field_tooltip(&path, &DataType::String, None, "String");
    assert!(missing_tooltip.starts_with("u.name: String\n"));
}

#[test]
fn evaluate_preview_waits_when_a_missing_input_breaks_rendering() {
    let inputs = vec![
        ("value".to_string(), Some(DataValue::from("world")), DataType::String),
        ("msg".to_string(), None, DataType::Any),
    ];
    assert_eq!(evaluate_preview("{{ value }} {{ msg.chat.id }}", &inputs), PreviewStatus::Waiting);
    let complete = vec![("value".to_string(), Some(DataValue::from("world")), DataType::String)];
    assert!(matches!(evaluate_preview("{{ value.a.b }}", &complete), PreviewStatus::RenderError(_)));
}
