use super::*;

#[test]
fn test_lint_unknown_function_with_suggestion() {
    let lints = lint("sqr(16)", &[]);
    assert_eq!(lints.len(), 1);
    assert_eq!(
        lints[0].message.key,
        "formula-lint-unknown-function-suggest"
    );
    assert!(lints[0].message.fallback.contains("Did you mean 'sqrt'?"));
}

#[test]
fn test_lint_unknown_function_without_suggestion() {
    let lints = lint("xyz123_unknown(16)", &[]);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "formula-lint-unknown-function");
}

#[test]
fn test_lint_wrong_arity() {
    let lints = lint("clamp(1, 2)", &[]);
    assert_eq!(lints.len(), 1);
    assert_eq!(lints[0].message.key, "formula-lint-wrong-arity");
    assert!(
        lints[0]
            .message
            .fallback
            .contains("expects 3 arguments, got 2")
    );
}

#[test]
fn test_lint_quantity_mismatch() {
    let inputs = vec![
        (
            "t".to_string(),
            DataType::Named("quantity.temperature".into()),
        ),
        ("l".to_string(), DataType::Named("quantity.length".into())),
    ];
    let lints = lint("t + l", &inputs);
    assert_eq!(lints.len(), 1);
    assert_eq!(
        lints[0].message.key,
        "error-formula-incompatible-quantities"
    );
}

#[test]
fn test_lint_clean_valid_formula() {
    let inputs = vec![("x".to_string(), DataType::Number)];
    let lints = lint("sqrt(x) + clamp(x, 0, 100)", &inputs);
    assert!(lints.is_empty());
}
