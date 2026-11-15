//! Unit tests for pure range and caret planning in undoable text editing.

use super::*;

#[test]
fn utf16_range_ascii() {
    let text = "hello world";
    assert_eq!(utf16_range(text, 0..5), (0, 5));
    assert_eq!(utf16_range(text, 6..11), (6, 11));
}

#[test]
fn utf16_range_cyrillic() {
    let text = "Привіт, світ!";
    // "Привіт" is 6 characters, 12 bytes in UTF-8, 6 UTF-16 code units
    assert_eq!(utf16_range(text, 0..12), (0, 6));
    // ", " is 2 bytes (12..14), 2 code units (6..8)
    // "світ!" is 5 characters (14..23 bytes), 5 code units (8..13)
    assert_eq!(utf16_range(text, 14..23), (8, 13));
}

#[test]
fn utf16_range_emoji_surrogate_pairs() {
    // '👋' is 4 bytes, 2 UTF-16 code units (surrogate pair)
    let text = "👋 world";
    assert_eq!(utf16_range(text, 0..4), (0, 2));
    assert_eq!(utf16_range(text, 5..10), (3, 8));
}

#[test]
fn plan_replace_template_auto_close_insert() {
    let current = "{";
    // User types second '{' at caret 1, inserting "{  }}"
    let (start_u16, end_u16, caret_u16, new_text) = plan_replace(current, 1..1, "{  }}", 3);
    assert_eq!(start_u16, 1);
    assert_eq!(end_u16, 1);
    assert_eq!(caret_u16, 3);
    assert_eq!(new_text, "{{  }}");
}

#[test]
fn plan_replace_template_auto_close_delete_pair() {
    let current = "{{  }}";
    // Backspacing pair: replace range 0..6 with "{"
    let (start_u16, end_u16, caret_u16, new_text) = plan_replace(current, 0..6, "{", 1);
    assert_eq!(start_u16, 0);
    assert_eq!(end_u16, 6);
    assert_eq!(caret_u16, 1);
    assert_eq!(new_text, "{");
}

#[test]
fn plan_replace_formula_wrap_parentheses() {
    let current = "1 + val";
    // User selected "val" (range 4..7) and pressed '(', wrapping in "(val)"
    let (start_u16, end_u16, caret_u16, new_text) = plan_replace(current, 4..7, "(val)", 9);
    assert_eq!(start_u16, 4);
    assert_eq!(end_u16, 7);
    assert_eq!(caret_u16, 9);
    assert_eq!(new_text, "1 + (val)");
}

#[test]
fn plan_replace_formula_delete_pair() {
    let current = "()";
    // User pressed Backspace inside empty parens (caret 1): range 0..2 replaced with ""
    let (start_u16, end_u16, caret_u16, new_text) = plan_replace(current, 0..2, "", 0);
    assert_eq!(start_u16, 0);
    assert_eq!(end_u16, 2);
    assert_eq!(caret_u16, 0);
    assert_eq!(new_text, "");
}

#[test]
fn plan_replace_cyrillic_context() {
    let current = "Привіт {";
    // "Привіт {" is 6 Cyrillic chars (12 bytes) + 1 space (1 byte) + 1 '{' (1 byte) = 14 bytes, 8 code units
    let (start_u16, end_u16, caret_u16, new_text) = plan_replace(current, 14..14, "{  }}", 16);
    assert_eq!(start_u16, 8);
    assert_eq!(end_u16, 8);
    assert_eq!(caret_u16, 10);
    assert_eq!(new_text, "Привіт {{  }}");
}

#[test]
fn plan_completion_plain_variable() {
    let current = "Hello {{ va }}";
    // Splicing "va" (range 9..11) with "value"
    let (start_u16, end_u16, caret_u16, new_text, new_caret_byte) =
        plan_completion(current, 9..11, "value", None);
    assert_eq!(start_u16, 9);
    assert_eq!(end_u16, 11);
    assert_eq!(caret_u16, 14);
    assert_eq!(new_caret_byte, 14);
    assert_eq!(new_text, "Hello {{ value }}");
}

#[test]
fn plan_completion_snippet_with_inner_caret() {
    let current = "{{ now() | da }}";
    // Splicing "da" (range 11..13) with "date('')", caret at offset 6 (inside quotes)
    let (start_u16, end_u16, caret_u16, new_text, new_caret_byte) =
        plan_completion(current, 11..13, "date('')", Some(6));
    assert_eq!(start_u16, 11);
    assert_eq!(end_u16, 13);
    assert_eq!(caret_u16, 17);
    assert_eq!(new_caret_byte, 17);
    assert_eq!(new_text, "{{ now() | date('') }}");
    assert_eq!(&new_text[16..18], "''");
}

#[test]
fn plan_completion_emoji_surrogate_pairs() {
    let current = "👋 {{ va }}";
    // "👋 " is 2 code units (emoji) + 1 code unit (space) = 3 code units
    // "{{ " is 3 code units -> "va" starts at u16 6
    // Byte offset: '👋' is 4 bytes + ' ' (1) + "{{ " (3) = 8 bytes
    // "va" is bytes 8..10
    let (start_u16, end_u16, caret_u16, new_text, new_caret_byte) =
        plan_completion(current, 8..10, "value", None);
    assert_eq!(start_u16, 6);
    assert_eq!(end_u16, 8);
    assert_eq!(caret_u16, 11);
    assert_eq!(new_caret_byte, 13);
    assert_eq!(new_text, "👋 {{ value }}");
}
