use super::*;

#[test]
fn test_sample_hi_name_upper() {
    let source = "Hi {{ name | upper }}";
    let inputs = vec!["name".to_string()];
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    assert_eq!(res.len(), 10);
    assert_eq!(
        res[0],
        Highlight {
            range: 0..3,
            class: TokenClass::Text
        }
    ); // "Hi "
    assert_eq!(
        res[1],
        Highlight {
            range: 3..5,
            class: TokenClass::Delimiter
        }
    ); // "{{"
    assert_eq!(
        res[2],
        Highlight {
            range: 5..6,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[3],
        Highlight {
            range: 6..10,
            class: TokenClass::Input
        }
    ); // "name"
    assert_eq!(
        res[4],
        Highlight {
            range: 10..11,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[5],
        Highlight {
            range: 11..12,
            class: TokenClass::Operator
        }
    ); // "|"
    assert_eq!(
        res[6],
        Highlight {
            range: 12..13,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[7],
        Highlight {
            range: 13..18,
            class: TokenClass::Filter
        }
    ); // "upper"
    assert_eq!(
        res[8],
        Highlight {
            range: 18..19,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[9],
        Highlight {
            range: 19..21,
            class: TokenClass::Delimiter
        }
    ); // "}}"
}

#[test]
fn test_sample_for_loop_with_field_and_endfor() {
    let source = "{% for x in items %}{{ x.id }}{% endfor %}";
    let inputs = Vec::new();
    let res = highlight(source, &inputs);
    assert_highlights_invariant(source, &res);

    // {% for x in items %}{{ x.id }}
    assert_eq!(
        res[0],
        Highlight {
            range: 0..2,
            class: TokenClass::Delimiter
        }
    ); // "{%"
    assert_eq!(
        res[1],
        Highlight {
            range: 2..3,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[2],
        Highlight {
            range: 3..6,
            class: TokenClass::Keyword
        }
    ); // "for"
    assert_eq!(
        res[3],
        Highlight {
            range: 6..7,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[4],
        Highlight {
            range: 7..8,
            class: TokenClass::Variable
        }
    ); // "x"
    assert_eq!(
        res[5],
        Highlight {
            range: 8..9,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[6],
        Highlight {
            range: 9..11,
            class: TokenClass::Keyword
        }
    ); // "in"
    assert_eq!(
        res[7],
        Highlight {
            range: 11..12,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[8],
        Highlight {
            range: 12..17,
            class: TokenClass::Variable
        }
    ); // "items"
    assert_eq!(
        res[9],
        Highlight {
            range: 17..18,
            class: TokenClass::Text
        }
    ); // " "
    // Note: "%}" (18..20) immediately followed by "{{" (20..22) are adjacent Delimiters and must be merged into 18..22
    assert_eq!(
        res[10],
        Highlight {
            range: 18..22,
            class: TokenClass::Delimiter
        }
    ); // "%}{{"

    // x.id }}
    assert_eq!(
        res[11],
        Highlight {
            range: 22..23,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[12],
        Highlight {
            range: 23..24,
            class: TokenClass::Variable
        }
    ); // "x"
    assert_eq!(
        res[13],
        Highlight {
            range: 24..25,
            class: TokenClass::Operator
        }
    ); // "."
    assert_eq!(
        res[14],
        Highlight {
            range: 25..27,
            class: TokenClass::Field
        }
    ); // "id"
    assert_eq!(
        res[15],
        Highlight {
            range: 27..28,
            class: TokenClass::Text
        }
    ); // " "
    // Note: "}}" (28..30) immediately followed by "{%" (30..32) are adjacent Delimiters and must be merged into 28..32
    assert_eq!(
        res[16],
        Highlight {
            range: 28..32,
            class: TokenClass::Delimiter
        }
    ); // "}}{%"

    // endfor %}
    assert_eq!(
        res[17],
        Highlight {
            range: 32..33,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[18],
        Highlight {
            range: 33..39,
            class: TokenClass::Keyword
        }
    ); // "endfor"
    assert_eq!(
        res[19],
        Highlight {
            range: 39..40,
            class: TokenClass::Text
        }
    ); // " "
    assert_eq!(
        res[20],
        Highlight {
            range: 40..42,
            class: TokenClass::Delimiter
        }
    ); // "%}"
}

#[test]
fn test_sample_is_defined_and_is_not() {
    let source = "{{ a is defined }}";
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
            range: 3..4,
            class: TokenClass::Variable
        }
    );
    assert_eq!(
        res[3],
        Highlight {
            range: 4..5,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[4],
        Highlight {
            range: 5..7,
            class: TokenClass::Keyword
        }
    ); // "is"
    assert_eq!(
        res[5],
        Highlight {
            range: 7..8,
            class: TokenClass::Text
        }
    );
    assert_eq!(
        res[6],
        Highlight {
            range: 8..15,
            class: TokenClass::Test
        }
    ); // "defined"
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

    let source2 = "{{ a is not defined }}";
    let res2 = highlight(source2, &[]);
    assert_highlights_invariant(source2, &res2);
    assert_eq!(
        res2[4],
        Highlight {
            range: 5..7,
            class: TokenClass::Keyword
        }
    ); // "is"
    assert_eq!(
        res2[6],
        Highlight {
            range: 8..11,
            class: TokenClass::Keyword
        }
    ); // "not"
    assert_eq!(
        res2[8],
        Highlight {
            range: 12..19,
            class: TokenClass::Test
        }
    ); // "defined"
}
