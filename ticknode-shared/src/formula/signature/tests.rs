use super::*;

#[test]
fn test_parse_signature_params() {
    assert_eq!(
        parse_signature_params("clamp(x, lo, hi)"),
        vec!["x", "lo", "hi"]
    );
    assert_eq!(
        parse_signature_params("round(x[, digits])"),
        vec!["x", "digits"]
    );
    assert_eq!(
        parse_signature_params("hypot(a, b, ...)"),
        vec!["a", "b", "..."]
    );
}

#[test]
fn test_signature_at_first_arg() {
    let source = "clamp(x";
    let help = signature_at(source, 7).unwrap();
    assert_eq!(help.name, "clamp");
    assert_eq!(help.active_param, 0);
    assert_eq!(help.params.len(), 3);
    assert_eq!(help.params[0].name, "x");
}

#[test]
fn test_signature_at_second_arg() {
    let source = "clamp(x, 10";
    let help = signature_at(source, 11).unwrap();
    assert_eq!(help.name, "clamp");
    assert_eq!(help.active_param, 1);
    assert_eq!(help.params[1].name, "lo");
}

#[test]
fn test_signature_at_nested_calls() {
    let source = "clamp(min(a, b), 10";
    // At caret right inside min(a
    let help_nested = signature_at(source, 10).unwrap();
    assert_eq!(help_nested.name, "min");
    assert_eq!(help_nested.active_param, 0);

    // At caret inside min(a, b
    let help_nested2 = signature_at(source, 13).unwrap();
    assert_eq!(help_nested2.name, "min");
    assert_eq!(help_nested2.active_param, 1);

    // At caret after min(...) has closed and after comma
    let help_outer = signature_at(source, 19).unwrap();
    assert_eq!(help_outer.name, "clamp");
    assert_eq!(help_outer.active_param, 1);
}

#[test]
fn test_signature_at_closed_call() {
    let source = "sqrt(16) + 1";
    assert!(signature_at(source, 12).is_none());
}
