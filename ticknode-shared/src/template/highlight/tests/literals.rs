use super::*;

#[test]
fn test_sample_comment() {
    let source = "{# c #}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);
    assert_eq!(res.len(), 1);
    assert_eq!(
        res[0],
        Highlight {
            range: 0..7,
            class: TokenClass::Comment
        }
    );

    let ws_comment = "{#- a comment -#}";
    let res_ws = highlight(ws_comment, &[]);
    assert_highlights_invariant(ws_comment, &res_ws);
    assert_eq!(res_ws.len(), 1);
    assert_eq!(
        res_ws[0],
        Highlight {
            range: 0..17,
            class: TokenClass::Comment
        }
    );
}

#[test]
fn test_sample_string_escape_and_float() {
    let source = "{{ 'a\\'b' ~ 1.5 }}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(
        res[0],
        Highlight {
            range: 0..2,
            class: TokenClass::Delimiter
        }
    );
    assert_eq!(
        res[1],
        Highlight {
            range: 2..3,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[2],
        Highlight {
            range: 3..9,
            class: TokenClass::String
        }
    ); // "'a\'b'"
    assert_eq!(
        res[3],
        Highlight {
            range: 9..10,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[4],
        Highlight {
            range: 10..11,
            class: TokenClass::Operator
        }
    ); // "~"
    assert_eq!(
        res[5],
        Highlight {
            range: 11..12,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[6],
        Highlight {
            range: 12..15,
            class: TokenClass::Number
        }
    ); // "1.5"
    assert_eq!(
        res[7],
        Highlight {
            range: 15..16,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[8],
        Highlight {
            range: 16..18,
            class: TokenClass::Delimiter
        }
    );
}

#[test]
fn test_sample_function_call() {
    let source = "{{ range(3) }}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(
        res[0],
        Highlight {
            range: 0..2,
            class: TokenClass::Delimiter
        }
    );
    assert_eq!(
        res[1],
        Highlight {
            range: 2..3,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[2],
        Highlight {
            range: 3..8,
            class: TokenClass::Function
        }
    ); // "range"
    assert_eq!(
        res[3],
        Highlight {
            range: 8..9,
            class: TokenClass::Operator
        }
    ); // "("
    assert_eq!(
        res[4],
        Highlight {
            range: 9..10,
            class: TokenClass::Number
        }
    ); // "3"
    assert_eq!(
        res[5],
        Highlight {
            range: 10..11,
            class: TokenClass::Operator
        }
    ); // ")"
    assert_eq!(
        res[6],
        Highlight {
            range: 11..12,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[7],
        Highlight {
            range: 12..14,
            class: TokenClass::Delimiter
        }
    );
}

#[test]
fn test_sample_unclosed_tag() {
    let source = "{{ na";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(
        res[0],
        Highlight {
            range: 0..2,
            class: TokenClass::Delimiter
        }
    );
    assert_eq!(
        res[1],
        Highlight {
            range: 2..3,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[2],
        Highlight {
            range: 3..5,
            class: TokenClass::Variable
        }
    );
}

#[test]
fn test_sample_cyrillic_text_before_tags() {
    let source = "Привіт {{ name }}";
    let inputs = vec!["name".to_string()];
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    // "Привіт " is 13 bytes
    assert_eq!(
        res[0],
        Highlight {
            range: 0..13,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[1],
        Highlight {
            range: 13..15,
            class: TokenClass::Delimiter
        }
    ); // "{{"
    assert_eq!(
        res[2],
        Highlight {
            range: 15..16,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[3],
        Highlight {
            range: 16..20,
            class: TokenClass::Input
        }
    ); // "name"
    assert_eq!(
        res[4],
        Highlight {
            range: 20..21,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[5],
        Highlight {
            range: 21..23,
            class: TokenClass::Delimiter
        }
    ); // "}}"
}

#[test]
fn test_whitespace_control_markers() {
    let source = "{{- foo -}}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);
    assert_eq!(
        res[0],
        Highlight {
            range: 0..3,
            class: TokenClass::Delimiter
        }
    ); // "{{-"
    assert_eq!(
        res[1],
        Highlight {
            range: 3..4,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[2],
        Highlight {
            range: 4..7,
            class: TokenClass::Variable
        }
    ); // "foo"
    assert_eq!(
        res[3],
        Highlight {
            range: 7..8,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[4],
        Highlight {
            range: 8..11,
            class: TokenClass::Delimiter
        }
    ); // "-}}"
}

#[test]
fn test_fuzz_truncations_invariant() {
    let templates = [
        "Hi {{ name | upper(1) }} {# comment #} {% for item in items %}{{ item.field }}{% endfor %}",
        "{{ 'escaped \\' quote' ~ \"double \\\" quote\" + 1.25e-3 // 2 ** 3 }}",
        "Привіт, світ! {{ a is not defined and b is true }} {% if x <= 10 %}yes{% else %}no{% endif %}",
        "{#- unclosed comment with -# symbols {# nested? #}",
        "{{{ a }}} { % not a tag % } {{",
        "{{ 0x1f + 0b101 - 0o77 * 42.0 }}",
        "",
        "just plain text without any tags at all",
        "{{",
        "{%",
        "{#",
        "{{ 'unclosed string",
        "{{ 123.",
    ];

    let inputs = vec!["name".to_string(), "items".to_string()];

    for template in &templates {
        // Test every char boundary prefix
        for (byte_idx, _) in template.char_indices() {
            let prefix = &template[..byte_idx];
            let res = highlight(prefix, &inputs);
            assert_highlights_invariant(prefix, &res);
        }
        // Test full template
        let res = highlight(template, &inputs);
        assert_highlights_invariant(template, &res);
    }
}
