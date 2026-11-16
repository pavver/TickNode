//! Pure unit tests for template field helpers (no browser required).

use super::caret::{apply_completion, byte_to_utf16, utf16_to_byte};
use ticknode_shared::{DataType, DataValue};

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

mod preview;

#[test]
fn apply_completion_variable_and_field() {
    let source = "Hello {{ va }}";
    // Splicing over "va" (range 9..11)
    let (res, caret) = apply_completion(source, 9..11, "value", None);
    assert_eq!(res, "Hello {{ value }}");
    assert_eq!(caret, 14); // right after "value"
}

#[test]
fn apply_completion_function_places_caret_between_parentheses() {
    let source = "{{ lip }}";
    // Splicing "lip" with "lipsum()"
    let (res, caret) = apply_completion(source, 3..6, "lipsum()", Some(7));
    assert_eq!(res, "{{ lipsum() }}");
    assert_eq!(caret, 10);
    assert_eq!(&res[9..11], "()");
}

#[test]
fn apply_completion_snippet_places_caret_inside_quotes() {
    let source = "{{ now() | da }}";
    // Splicing "da" (range 11..13) with "date('')", caret inside quotes at offset 6
    let (res, caret) = apply_completion(source, 11..13, "date('')", Some(6));
    assert_eq!(res, "{{ now() | date('') }}");
    assert_eq!(caret, 17);
    assert_eq!(&res[16..18], "''");
}

#[test]
fn undoable_plan_replace_computes_utf16_offsets() {
    use crate::inline::undoable;
    let source = "Привіт {{ va }}";
    // "va" is at byte 16..18 (Cyrillic "Привіт" is 6 chars / 12 bytes + " {{ " is 4 bytes)
    // UTF-16: 6 code units ("Привіт") + 1 (" ") + 2 ("{{") + 1 (" ") = 10 code units
    let (start_u16, end_u16, caret_u16, new_text) =
        undoable::plan_replace(source, 16..18, "value", 21);
    assert_eq!(start_u16, 10);
    assert_eq!(end_u16, 12);
    assert_eq!(caret_u16, 15);
    assert_eq!(new_text, "Привіт {{ value }}");
}

#[test]
fn trigger_chars_identify_completion_positions() {
    use super::completion::is_completion_trigger_char;

    assert!(is_completion_trigger_char("{{ v", false));
    assert!(is_completion_trigger_char("{{ a.", false));
    assert!(is_completion_trigger_char("{{ a|", false));
    assert!(is_completion_trigger_char("{{ a | ", false));
    assert!(!is_completion_trigger_char("{{ a || ", false));
    assert!(is_completion_trigger_char("{{ a | date( ", false));
    assert!(is_completion_trigger_char("{{ a | date('x', ", false));
    assert!(is_completion_trigger_char("date('", false));
    assert!(is_completion_trigger_char("date(\"", false));
    assert!(is_completion_trigger_char("%", false));
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
    use super::hover::{HoverToken, token_at};
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
    use super::hover::{HoverToken, token_at};
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
    use super::hover::{HoverToken, token_at};
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
    use super::hover::{HoverToken, token_at};
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
    use ticknode_shared::TypeRegistry;
    use ticknode_shared::types::{FieldDefinition, TypeDefinition, TypeKind};

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
            data_type: None,
            value: DataValue::from("Alice"),
        },
    );
    fields.insert(
        "age".to_string(),
        ticknode_shared::ObjectField {
            data_type: None,
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
fn test_clamp_error_range_bounds_and_char_boundaries() {
    use super::highlight::clamp_error_range;

    assert_eq!(clamp_error_range("", 0..0), None);

    let ascii = "hello";
    assert_eq!(clamp_error_range(ascii, 1..3), Some(1..3));
    assert_eq!(clamp_error_range(ascii, 2..2), Some(2..3));
    // At end-of-text, marks last char
    assert_eq!(clamp_error_range(ascii, 5..5), Some(4..5));
    assert_eq!(clamp_error_range(ascii, 10..20), Some(4..5));

    let cyrillic = "Привіт"; // 6 chars, 12 bytes
    // Char 1 'р' is bytes 2..4
    assert_eq!(clamp_error_range(cyrillic, 2..4), Some(2..4));
    // Unaligned offset 3 aligns to char boundary
    assert_eq!(clamp_error_range(cyrillic, 3..3), Some(2..4));
}

#[test]
fn test_compute_styled_spans_with_error() {
    use super::highlight::compute_styled_spans;
    use ticknode_shared::template::highlight;

    let source = "Hi {{ unclosed";
    let highlights = highlight(source, &[]);
    // Error on "unclosed" (6..14)
    let spans = compute_styled_spans(source, &highlights, Some(6..14), &[]);
    assert_eq!(spans[0].text, "Hi ");
    assert_eq!(spans[0].class, "tk-text");
    assert!(!spans[0].is_error);
    assert!(!spans[0].is_lint);

    assert_eq!(spans[1].text, "{{");
    assert_eq!(spans[1].class, "tk-delimiter");
    assert!(!spans[1].is_error);
    assert!(!spans[1].is_lint);

    assert_eq!(spans[2].text, " ");
    assert_eq!(spans[2].class, "tk-text");
    assert!(!spans[2].is_error);
    assert!(!spans[2].is_lint);

    assert_eq!(spans[3].text, "unclosed");
    assert_eq!(spans[3].class, "tk-variable");
    assert!(spans[3].is_error);
    assert!(!spans[3].is_lint);
}

#[test]
fn test_compute_styled_spans_with_lint() {
    use super::highlight::compute_styled_spans;
    use ticknode_shared::template::highlight;

    let source = "Hi {{ x | abss }}";
    let highlights = highlight(source, &[]);
    // Lint on "abss" (10..14)
    let spans = compute_styled_spans(source, &highlights, None, &[10..14]);
    let lint_span = spans.iter().find(|s| s.text == "abss").expect("find abss");
    assert_eq!(lint_span.class, "tk-filter");
    assert!(!lint_span.is_error);
    assert!(lint_span.is_lint);
}

#[test]
fn test_hover_filter_and_function_docs() {
    use ticknode_shared::template::complete::docs::{DocKind, find_doc};

    let date_doc = find_doc(DocKind::Filter, "date").expect("date doc exists");
    assert_eq!(date_doc.signature, "date([pattern])");
    assert_eq!(date_doc.help_key, "template-help-filter-date");

    let range_doc = find_doc(DocKind::Function, "range").expect("range doc exists");
    assert_eq!(range_doc.signature, "range([start], stop, [step])");
    assert_eq!(range_doc.help_key, "template-help-function-range");

    let div_doc = find_doc(DocKind::Test, "divisibleby").expect("divisibleby doc exists");
    assert_eq!(div_doc.signature, "is divisibleby(num)");
    assert_eq!(div_doc.help_key, "template-help-test-divisibleby");
}
