//! Unit tests for signature help evaluation.

use super::*;
use crate::template::complete::docs::DocKind;

#[test]
fn test_signature_simple_function() {
    let src = "{{ range(1, 10) }}";
    // Caret at '1': active_param 0
    let help = signature_at(src, 9).expect("should find range");
    assert_eq!(help.name, "range");
    assert_eq!(help.kind, DocKind::Function);
    assert_eq!(help.active_param, 0);

    // Caret at '10': active_param 1
    let help = signature_at(src, 13).expect("should find range");
    assert_eq!(help.name, "range");
    assert_eq!(help.active_param, 1);
}

#[test]
fn test_signature_nested_parentheses() {
    let src = "{{ date(format(a, b), c) }}";
    // Inside format at 'a': active call is format, active_param 0
    let help = signature_at(src, 15).expect("should find format");
    assert_eq!(help.name, "format");
    assert_eq!(help.active_param, 0);

    // Inside format at 'b': active call is format, active_param 1
    let help = signature_at(src, 18).expect("should find format");
    assert_eq!(help.name, "format");
    assert_eq!(help.active_param, 1);

    // At 'c': format was closed, active call is date, active_param 1
    let help = signature_at(src, 23).expect("should find date");
    assert_eq!(help.name, "date");
    assert_eq!(help.active_param, 1);
}

#[test]
fn test_signature_strings_with_commas() {
    let src = "{{ date('2026, 11, 14', pattern) }}";
    // Caret inside the date string: active_param 0
    let help = signature_at(src, 15).expect("should find date");
    assert_eq!(help.name, "date");
    assert_eq!(help.active_param, 0);

    // Caret inside pattern: active_param 1
    let help = signature_at(src, 27).expect("should find date");
    assert_eq!(help.name, "date");
    assert_eq!(help.active_param, 1);
}

#[test]
fn test_signature_keyword_args() {
    let src = "{{ round(method='ceil') }}";
    // Inside method='ceil': selects parameter 'method' (index 1)
    let help = signature_at(src, 16).expect("should find round");
    assert_eq!(help.name, "round");
    assert_eq!(help.active_param, 1);

    let src2 = "{{ unit('C', digits=2) }}";
    let help2 = signature_at(src2, 20).expect("should find unit");
    assert_eq!(help2.name, "unit");
    assert_eq!(help2.active_param, 1);
}

#[test]
fn test_signature_tests_with_and_without_parens() {
    // With parens
    let src1 = "{{ x is divisibleby(3) }}";
    let help1 = signature_at(src1, 20).expect("should find divisibleby");
    assert_eq!(help1.name, "divisibleby");
    assert_eq!(help1.kind, DocKind::Test);
    assert_eq!(help1.active_param, 0);

    // Without parens
    let src2 = "{{ x is divisibleby 3 }}";
    let help2 = signature_at(src2, 21).expect("should find divisibleby");
    assert_eq!(help2.name, "divisibleby");
    assert_eq!(help2.kind, DocKind::Test);
    assert_eq!(help2.active_param, 0);

    // `is not` without parens
    let src3 = "{{ x is not endingwith 'foo' }}";
    let help3 = signature_at(src3, 24).expect("should find endingwith");
    assert_eq!(help3.name, "endingwith");
    assert_eq!(help3.kind, DocKind::Test);
    assert_eq!(help3.active_param, 0);
}

#[test]
fn test_signature_caret_outside_returns_none() {
    // Plain text outside tags
    assert!(signature_at("Hello world", 5).is_none());

    // Expression without call
    assert!(signature_at("{{ 1 + 2 }}", 5).is_none());

    // Caret on the function name before '('
    assert!(signature_at("{{ date('foo') }}", 4).is_none());

    // Caret right after closing ')'
    assert!(signature_at("{{ date('foo') }}", 14).is_none());

    // Inside comment
    assert!(signature_at("{# comment with date() #}", 10).is_none());

    // Inside string literal not part of any call
    assert!(signature_at("{{ 'date(1, 2)' }}", 6).is_none());
}
