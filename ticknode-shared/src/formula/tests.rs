mod functions;
mod inference;

use super::*;
use crate::NumberValue;
use std::collections::HashMap;

fn eval(source: &str) -> Result<FormulaValue, FormulaError> {
    let compiled = compile(source)?;
    compiled.eval(&|_| None)
}

fn eval_with(source: &str, vars: &[(&str, FormulaValue)]) -> Result<FormulaValue, FormulaError> {
    let map: HashMap<&str, FormulaValue> = vars.iter().cloned().collect();
    let compiled = compile(source)?;
    compiled.eval(&|name| map.get(name).cloned())
}

#[test]
fn test_number_literals_and_constants() {
    assert_eq!(
        eval("42").unwrap(),
        FormulaValue::Number(NumberValue::Int(42))
    );
    assert_eq!(
        eval("1_000_000").unwrap(),
        FormulaValue::Number(NumberValue::Int(1_000_000))
    );
    assert_eq!(
        eval("3.25").unwrap(),
        FormulaValue::Number(NumberValue::Float(3.25))
    );
    assert_eq!(
        eval("1_000.50_001").unwrap(),
        FormulaValue::Number(NumberValue::Float(1000.50001))
    );
    assert_eq!(
        eval("1e-3").unwrap(),
        FormulaValue::Number(NumberValue::Float(0.001))
    );
    assert_eq!(
        eval("2.5E+2").unwrap(),
        FormulaValue::Number(NumberValue::Int(250))
    );

    assert_eq!(eval("true").unwrap(), FormulaValue::Bool(true));
    assert_eq!(eval("false").unwrap(), FormulaValue::Bool(false));

    assert_eq!(
        eval("pi").unwrap(),
        FormulaValue::Number(NumberValue::Float(std::f64::consts::PI))
    );
    assert_eq!(
        eval("e").unwrap(),
        FormulaValue::Number(NumberValue::Float(std::f64::consts::E))
    );
    assert_eq!(
        eval("tau").unwrap(),
        FormulaValue::Number(NumberValue::Float(std::f64::consts::TAU))
    );
}

#[test]
fn test_operator_precedence_and_associativity() {
    // Arithmetic precedence
    assert_eq!(
        eval("2 + 3 * 4").unwrap(),
        FormulaValue::Number(NumberValue::Int(14))
    );
    assert_eq!(
        eval("(2 + 3) * 4").unwrap(),
        FormulaValue::Number(NumberValue::Int(20))
    );
    assert_eq!(
        eval("10 - 4 - 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    ); // left-associative

    // Power right-associative
    assert_eq!(
        eval("2 ^ 3 ^ 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(512))
    );
    assert_eq!(
        eval("2 ** 3 ** 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(512))
    );
    assert_eq!(
        eval("(2 ^ 3) ^ 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(64))
    );

    // Unary precedence
    assert_eq!(
        eval("-2 ^ 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(-4))
    );
    assert_eq!(
        eval("(-2) ^ 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    );
    assert_eq!(eval("!true").unwrap(), FormulaValue::Bool(false));
    assert_eq!(eval("not false").unwrap(), FormulaValue::Bool(true));

    // Logic operators and aliases
    assert_eq!(
        eval("true || false && false").unwrap(),
        FormulaValue::Bool(true)
    );
    assert_eq!(
        eval("false && true || true").unwrap(),
        FormulaValue::Bool(true)
    );
    assert_eq!(
        eval("true or false and false").unwrap(),
        FormulaValue::Bool(true)
    );

    // Ternary operator
    assert_eq!(
        eval("true ? 10 : 20").unwrap(),
        FormulaValue::Number(NumberValue::Int(10))
    );
    assert_eq!(
        eval("false ? 10 : 20").unwrap(),
        FormulaValue::Number(NumberValue::Int(20))
    );
    // Ternary right-associative
    assert_eq!(
        eval("false ? 1 : true ? 2 : 3").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
}

#[test]
fn test_evalexpr_compatibility() {
    assert_eq!(
        eval_with(
            "(a + b) / 2",
            &[
                ("a", FormulaValue::Number(NumberValue::Int(10))),
                ("b", FormulaValue::Number(NumberValue::Int(20))),
            ]
        )
        .unwrap(),
        FormulaValue::Number(NumberValue::Int(15))
    );

    assert_eq!(
        eval_with(
            "x * x - 1",
            &[("x", FormulaValue::Number(NumberValue::Int(5))),]
        )
        .unwrap(),
        FormulaValue::Number(NumberValue::Int(24))
    );

    // Qualified function names
    assert_eq!(
        eval("math::sqrt(16)").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    );
    assert_eq!(
        eval("math::min(10, 20)").unwrap(),
        FormulaValue::Number(NumberValue::Int(10))
    );
    assert_eq!(
        eval("math::max(10, 20)").unwrap(),
        FormulaValue::Number(NumberValue::Int(20))
    );
    assert_eq!(
        eval("math::abs(-7)").unwrap(),
        FormulaValue::Number(NumberValue::Int(7))
    );
}

#[test]
fn test_integer_exactness_and_overflow() {
    // Exact division
    assert_eq!(
        eval("10 / 2").unwrap(),
        FormulaValue::Number(NumberValue::Int(5))
    );
    // Fractional division
    assert_eq!(
        eval("10 / 4").unwrap(),
        FormulaValue::Number(NumberValue::Float(2.5))
    );

    // Overflow to float
    let max_i64 = i64::MAX;
    let expr = format!("{max_i64} + 1");
    let res = eval(&expr).unwrap();
    assert!(matches!(res, FormulaValue::Number(NumberValue::Float(_))));
}

#[test]
fn test_division_by_zero_and_runtime_errors() {
    assert!(
        matches!(eval("10 / 0"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-division-by-zero")
    );
    assert!(
        matches!(eval("10 % 0"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-division-by-zero")
    );
    assert!(
        matches!(eval("10.0 / 0.0"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-division-by-zero")
    );
}

#[test]
fn test_variables_extraction() {
    assert_eq!(
        variables("x * y + x * z - a").unwrap(),
        vec![
            "x".to_string(),
            "y".to_string(),
            "z".to_string(),
            "a".to_string()
        ]
    );

    // Constants and functions are excluded
    assert_eq!(
        variables("sin(x) + cos(y) * pi + e").unwrap(),
        vec!["x".to_string(), "y".to_string()]
    );

    // Variable references are excluded from input pins
    assert_eq!(variables("var.my_var + 1").unwrap(), Vec::<String>::new());
    assert_eq!(variables("sheet.x * 2 + a").unwrap(), vec!["a".to_string()]);

    // Plugin paths are read-only references, never input pins.
    assert!(variables("other.my_var + 1").unwrap().is_empty());
}

#[test]
fn test_source_limits_and_errors() {
    // Source size limit
    let huge = "1 + ".repeat(2000);
    assert!(matches!(
        compile(&huge),
        Err(FormulaError { ref message, .. }) if message.key == "error-formula-too-large"
    ));

    // Nesting depth limit
    let deep = "(".repeat(70) + "1" + &")".repeat(70);
    assert!(matches!(
        compile(&deep),
        Err(FormulaError { ref message, .. }) if message.key == "error-formula-depth-exceeded"
    ));

    // Error range check
    let err = compile("1 + * 2").unwrap_err();
    assert_eq!(err.range, 4..5);
    assert_eq!(err.message.key, "error-formula-unexpected-token");

    // Single &, |, = errors
    assert!(compile("1 & 2").is_err());
    assert!(compile("1 | 2").is_err());
    assert!(compile("1 = 2").is_err());
    assert!(compile("1 @ 2").is_err());

    // Scientific notation
    let f1 = compile("1e2 + 1.5e-1").unwrap();
    assert_eq!(
        f1.eval(&|_| None).unwrap(),
        FormulaValue::Number(100.15.into())
    );
}

#[test]
fn test_formula_variable_evaluation() {
    let f = compile("var.x * 2 + sheet.y").unwrap();
    let vars = variables("var.x * 2 + sheet.y").unwrap();
    assert!(vars.is_empty(), "formula variables should not become pins");

    let val = f
        .eval_with(&|_| None, &|vref| match vref.name.as_str() {
            "x" => Ok(FormulaValue::Number(10.into())),
            "y" => Ok(FormulaValue::Number(5.into())),
            _ => Err(super::error::FormulaError::new(0..0, "err", "unknown")),
        })
        .unwrap();

    assert_eq!(val, FormulaValue::Number(25.into()));
}

#[test]
fn test_formula_comparisons_and_equality() {
    let check_bool = |expr: &str, expected: bool| {
        let f = compile(expr).unwrap();
        assert_eq!(f.eval(&|_| None).unwrap(), FormulaValue::Bool(expected));
    };

    check_bool("1 == 1", true);
    check_bool("1 == 2", false);
    check_bool("1.5 == 1.5", true);
    check_bool("1.5 == 2.5", false);
    check_bool("true == true", true);
    check_bool("true == false", false);
    check_bool("1 != 2", true);
    check_bool("1 != 1", false);
    check_bool("1.5 != 2.5", true);
    check_bool("1.5 != 1.5", false);
    check_bool("true != false", true);
    check_bool("true != true", false);

    check_bool("1 < 2", true);
    check_bool("2 < 2", false);
    check_bool("2 <= 2", true);
    check_bool("3 <= 2", false);
    check_bool("3 > 2", true);
    check_bool("2 > 2", false);
    check_bool("2 >= 2", true);
    check_bool("1 >= 2", false);

    check_bool("!true", false);
    check_bool("!false", true);
    check_bool("not (1 == 2)", true);

    check_bool("false && (1 / 0 == 0)", false);
    check_bool("true || (1 / 0 == 0)", true);
    check_bool("false and (1 / 0 == 0)", false);
    check_bool("true or (1 / 0 == 0)", true);
}

#[test]
fn test_formula_value_conversions_and_fallbacks() {
    use crate::DataValue;
    assert_eq!(
        DataValue::from(FormulaValue::Number(42.into())),
        DataValue::Number(42.into())
    );
    assert_eq!(
        DataValue::from(FormulaValue::Bool(true)),
        DataValue::Bool(true)
    );

    let f = compile("var.missing").unwrap();
    let err = f.eval(&|_| None).unwrap_err();
    assert_eq!(err.message.key, "error-variable-missing");
}
