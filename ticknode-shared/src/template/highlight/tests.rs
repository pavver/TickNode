use super::*;

fn assert_highlights_invariant(source: &str, highlights: &[Highlight]) {
    if source.is_empty() {
        assert!(highlights.is_empty());
        return;
    }
    assert!(!highlights.is_empty());
    assert_eq!(highlights[0].range.start, 0);
    assert_eq!(highlights.last().unwrap().range.end, source.len());

    for i in 0..highlights.len() {
        let h = &highlights[i];
        assert!(
            h.range.start < h.range.end,
            "range must not be empty at index {i}: {:?}",
            h.range
        );
        assert!(
            source.is_char_boundary(h.range.start),
            "start {} must be a char boundary",
            h.range.start
        );
        assert!(
            source.is_char_boundary(h.range.end),
            "end {} must be a char boundary",
            h.range.end
        );

        if i > 0 {
            let prev = &highlights[i - 1];
            assert_eq!(
                prev.range.end, h.range.start,
                "adjacent ranges must be contiguous: {:?} then {:?}",
                prev, h
            );
            assert_ne!(
                prev.class, h.class,
                "adjacent ranges of identical class must be merged: {:?} then {:?}",
                prev, h
            );
        }
    }
}

#[test]
fn test_sample_hi_name_upper() {
    let source = "Hi {{ name | upper }}";
    let inputs = vec!["name".to_string()];
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    assert_eq!(res.len(), 10);
    assert_eq!(res[0], Highlight { range: 0..3, class: TokenClass::Text }); // "Hi "
    assert_eq!(res[1], Highlight { range: 3..5, class: TokenClass::Delimiter }); // "{{"
    assert_eq!(res[2], Highlight { range: 5..6, class: TokenClass::Text }); // " "
    assert_eq!(res[3], Highlight { range: 6..10, class: TokenClass::Input }); // "name"
    assert_eq!(res[4], Highlight { range: 10..11, class: TokenClass::Text }); // " "
    assert_eq!(res[5], Highlight { range: 11..12, class: TokenClass::Operator }); // "|"
    assert_eq!(res[6], Highlight { range: 12..13, class: TokenClass::Text }); // " "
    assert_eq!(res[7], Highlight { range: 13..18, class: TokenClass::Filter }); // "upper"
    assert_eq!(res[8], Highlight { range: 18..19, class: TokenClass::Text }); // " "
    assert_eq!(res[9], Highlight { range: 19..21, class: TokenClass::Delimiter }); // "}}"
}

#[test]
fn test_sample_for_loop_with_field_and_endfor() {
    let source = "{% for x in items %}{{ x.id }}{% endfor %}";
    let inputs = Vec::new();
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    // {% for x in items %}{{ x.id }}
    assert_eq!(res[0], Highlight { range: 0..2, class: TokenClass::Delimiter }); // "{%"
    assert_eq!(res[1], Highlight { range: 2..3, class: TokenClass::Text }); // " "
    assert_eq!(res[2], Highlight { range: 3..6, class: TokenClass::Keyword }); // "for"
    assert_eq!(res[3], Highlight { range: 6..7, class: TokenClass::Text }); // " "
    assert_eq!(res[4], Highlight { range: 7..8, class: TokenClass::Variable }); // "x"
    assert_eq!(res[5], Highlight { range: 8..9, class: TokenClass::Text }); // " "
    assert_eq!(res[6], Highlight { range: 9..11, class: TokenClass::Keyword }); // "in"
    assert_eq!(res[7], Highlight { range: 11..12, class: TokenClass::Text }); // " "
    assert_eq!(res[8], Highlight { range: 12..17, class: TokenClass::Variable }); // "items"
    assert_eq!(res[9], Highlight { range: 17..18, class: TokenClass::Text }); // " "
    // Note: "%}" (18..20) immediately followed by "{{" (20..22) are adjacent Delimiters and must be merged into 18..22
    assert_eq!(res[10], Highlight { range: 18..22, class: TokenClass::Delimiter }); // "%}{{"

    // x.id }}
    assert_eq!(res[11], Highlight { range: 22..23, class: TokenClass::Text }); // " "
    assert_eq!(res[12], Highlight { range: 23..24, class: TokenClass::Variable }); // "x"
    assert_eq!(res[13], Highlight { range: 24..25, class: TokenClass::Operator }); // "."
    assert_eq!(res[14], Highlight { range: 25..27, class: TokenClass::Field }); // "id"
    assert_eq!(res[15], Highlight { range: 27..28, class: TokenClass::Text }); // " "
    // Note: "}}" (28..30) immediately followed by "{%" (30..32) are adjacent Delimiters and must be merged into 28..32
    assert_eq!(res[16], Highlight { range: 28..32, class: TokenClass::Delimiter }); // "}}{%"

    // endfor %}
    assert_eq!(res[17], Highlight { range: 32..33, class: TokenClass::Text }); // " "
    assert_eq!(res[18], Highlight { range: 33..39, class: TokenClass::Keyword }); // "endfor"
    assert_eq!(res[19], Highlight { range: 39..40, class: TokenClass::Text }); // " "
    assert_eq!(res[20], Highlight { range: 40..42, class: TokenClass::Delimiter }); // "%}"
}

#[test]
fn test_sample_is_defined_and_is_not() {
    let source = "{{ a is defined }}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(res[0], Highlight { range: 0..2, class: TokenClass::Delimiter });
    assert_eq!(res[1], Highlight { range: 2..3, class: TokenClass::Text });
    assert_eq!(res[2], Highlight { range: 3..4, class: TokenClass::Variable });
    assert_eq!(res[3], Highlight { range: 4..5, class: TokenClass::Text });
    assert_eq!(res[4], Highlight { range: 5..7, class: TokenClass::Keyword }); // "is"
    assert_eq!(res[5], Highlight { range: 7..8, class: TokenClass::Text });
    assert_eq!(res[6], Highlight { range: 8..15, class: TokenClass::Test }); // "defined"
    assert_eq!(res[7], Highlight { range: 15..16, class: TokenClass::Text });
    assert_eq!(res[8], Highlight { range: 16..18, class: TokenClass::Delimiter });

    let source2 = "{{ a is not defined }}";
    let res2 = highlight(source2, &[]);
    assert_highlights_invariant(source2, &res2);
    assert_eq!(res2[4], Highlight { range: 5..7, class: TokenClass::Keyword }); // "is"
    assert_eq!(res2[6], Highlight { range: 8..11, class: TokenClass::Keyword }); // "not"
    assert_eq!(res2[8], Highlight { range: 12..19, class: TokenClass::Test }); // "defined"
}

#[test]
fn test_sample_comment() {
    let source = "{# c #}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);
    assert_eq!(res.len(), 1);
    assert_eq!(res[0], Highlight { range: 0..7, class: TokenClass::Comment });

    let ws_comment = "{#- a comment -#}";
    let res_ws = highlight(ws_comment, &[]);
    assert_highlights_invariant(ws_comment, &res_ws);
    assert_eq!(res_ws.len(), 1);
    assert_eq!(res_ws[0], Highlight { range: 0..17, class: TokenClass::Comment });
}

#[test]
fn test_sample_string_escape_and_float() {
    let source = "{{ 'a\\'b' ~ 1.5 }}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(res[0], Highlight { range: 0..2, class: TokenClass::Delimiter });
    assert_eq!(res[1], Highlight { range: 2..3, class: TokenClass::Text });
    assert_eq!(res[2], Highlight { range: 3..9, class: TokenClass::String }); // "'a\'b'"
    assert_eq!(res[3], Highlight { range: 9..10, class: TokenClass::Text });
    assert_eq!(res[4], Highlight { range: 10..11, class: TokenClass::Operator }); // "~"
    assert_eq!(res[5], Highlight { range: 11..12, class: TokenClass::Text });
    assert_eq!(res[6], Highlight { range: 12..15, class: TokenClass::Number }); // "1.5"
    assert_eq!(res[7], Highlight { range: 15..16, class: TokenClass::Text });
    assert_eq!(res[8], Highlight { range: 16..18, class: TokenClass::Delimiter });
}

#[test]
fn test_sample_function_call() {
    let source = "{{ range(3) }}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(res[0], Highlight { range: 0..2, class: TokenClass::Delimiter });
    assert_eq!(res[1], Highlight { range: 2..3, class: TokenClass::Text });
    assert_eq!(res[2], Highlight { range: 3..8, class: TokenClass::Function }); // "range"
    assert_eq!(res[3], Highlight { range: 8..9, class: TokenClass::Operator }); // "("
    assert_eq!(res[4], Highlight { range: 9..10, class: TokenClass::Number }); // "3"
    assert_eq!(res[5], Highlight { range: 10..11, class: TokenClass::Operator }); // ")"
    assert_eq!(res[6], Highlight { range: 11..12, class: TokenClass::Text });
    assert_eq!(res[7], Highlight { range: 12..14, class: TokenClass::Delimiter });
}

#[test]
fn test_sample_unclosed_tag() {
    let source = "{{ na";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);

    assert_eq!(res[0], Highlight { range: 0..2, class: TokenClass::Delimiter });
    assert_eq!(res[1], Highlight { range: 2..3, class: TokenClass::Text });
    assert_eq!(res[2], Highlight { range: 3..5, class: TokenClass::Variable });
}

#[test]
fn test_sample_cyrillic_text_before_tags() {
    let source = "Привіт {{ name }}";
    let inputs = vec!["name".to_string()];
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    // "Привіт " is 13 bytes
    assert_eq!(res[0], Highlight { range: 0..13, class: TokenClass::Text });
    assert_eq!(res[1], Highlight { range: 13..15, class: TokenClass::Delimiter }); // "{{"
    assert_eq!(res[2], Highlight { range: 15..16, class: TokenClass::Text });
    assert_eq!(res[3], Highlight { range: 16..20, class: TokenClass::Input }); // "name"
    assert_eq!(res[4], Highlight { range: 20..21, class: TokenClass::Text });
    assert_eq!(res[5], Highlight { range: 21..23, class: TokenClass::Delimiter }); // "}}"
}

#[test]
fn test_whitespace_control_markers() {
    let source = "{{- foo -}}";
    let res = highlight(source, &[]);
    assert_highlights_invariant(source, &res);
    assert_eq!(res[0], Highlight { range: 0..3, class: TokenClass::Delimiter }); // "{{-"
    assert_eq!(res[1], Highlight { range: 3..4, class: TokenClass::Text });
    assert_eq!(res[2], Highlight { range: 4..7, class: TokenClass::Variable }); // "foo"
    assert_eq!(res[3], Highlight { range: 7..8, class: TokenClass::Text });
    assert_eq!(res[4], Highlight { range: 8..11, class: TokenClass::Delimiter }); // "-}}"
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
