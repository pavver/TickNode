use super::*;

#[test]
fn test_highlight_complete_coverage() {
    let source = "clamp(x + 42.5 * pi, 0, 100) != nan";
    let inputs = vec!["x".to_string()];
    let highlights = highlight(source, &inputs);

    // Verify contiguous coverage with no gaps or overlaps
    let mut cursor = 0;
    for h in &highlights {
        assert_eq!(h.range.start, cursor);
        assert!(h.range.end > h.range.start);
        cursor = h.range.end;
    }
    assert_eq!(cursor, source.len());
}

#[test]
fn test_token_classes() {
    let source = "sin(x) + 42 * pi && true ? 1 : 0";
    let inputs = vec!["x".to_string()];
    let highlights = highlight(source, &inputs);

    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Function && &source[h.range.clone()] == "sin")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Input && &source[h.range.clone()] == "x")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Number && &source[h.range.clone()] == "42")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Constant && &source[h.range.clone()] == "pi")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Constant && &source[h.range.clone()] == "true")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "+")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "&&")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "?")
    );
}

#[test]
fn test_highlight_error_token() {
    let source = "x @ 5";
    let inputs = vec!["x".to_string()];
    let highlights = highlight(source, &inputs);
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Error && &source[h.range.clone()] == "@")
    );
}

#[test]
fn test_highlight_operator_keywords() {
    let source = "a and not b or c";
    let inputs = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let highlights = highlight(source, &inputs);

    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "and")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "not")
    );
    assert!(
        highlights
            .iter()
            .any(|h| h.class == TokenClass::Operator && &source[h.range.clone()] == "or")
    );
}
