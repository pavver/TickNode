use super::super::*;
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
fn test_basic_math_functions() {
    assert_eq!(
        eval("abs(-5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(5))
    );
    assert_eq!(
        eval("abs(3.5)").unwrap(),
        FormulaValue::Number(NumberValue::Float(3.5))
    );
    assert_eq!(
        eval("sign(-10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(-1))
    );
    assert_eq!(
        eval("sign(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("sign(10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );

    assert_eq!(
        eval("min(5, 2, 8, 1)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("max(5, 2, 8, 1)").unwrap(),
        FormulaValue::Number(NumberValue::Int(8))
    );
    assert_eq!(
        eval("clamp(15, 0, 10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(10))
    );
    assert_eq!(
        eval("clamp(-5, 0, 10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("clamp(5, 0, 10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(5))
    );

    assert_eq!(
        eval("round(2.6)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert_eq!(
        eval("round(2.345, 2)").unwrap(),
        FormulaValue::Number(NumberValue::Float(2.35))
    );
    assert_eq!(
        eval("floor(2.9)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("ceil(2.1)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert_eq!(
        eval("trunc(-2.7)").unwrap(),
        FormulaValue::Number(NumberValue::Int(-2))
    );
    assert_eq!(
        eval("frac(2.75)").unwrap(),
        FormulaValue::Number(NumberValue::Float(0.75))
    );
}

#[test]
fn test_roots_and_exponentials() {
    assert_eq!(
        eval("sqrt(9)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert_eq!(
        eval("sqrt(2)").unwrap(),
        FormulaValue::Number(NumberValue::Float(std::f64::consts::SQRT_2))
    );
    assert!(eval("sqrt(-1)").is_err());

    assert_eq!(
        eval("cbrt(27)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert_eq!(
        eval("pow(2, 3)").unwrap(),
        FormulaValue::Number(NumberValue::Int(8))
    );
    assert_eq!(
        eval("exp(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );

    assert_eq!(
        eval("ln(e)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("log(100, 10)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("log(e)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("log10(1000)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert_eq!(
        eval("log2(8)").unwrap(),
        FormulaValue::Number(NumberValue::Int(3))
    );
    assert!(eval("ln(0)").is_err());
    assert!(eval("log10(-5)").is_err());
}

#[test]
fn test_trigonometry_and_angles() {
    assert_eq!(
        eval("sin(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("cos(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("tan(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );

    assert_eq!(
        eval("asin(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("acos(1)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("atan(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("atan2(0, 1)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert!(eval("asin(2)").is_err());

    assert_eq!(
        eval("sinh(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("cosh(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("tanh(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );

    assert_eq!(
        eval("deg(pi)").unwrap(),
        FormulaValue::Number(NumberValue::Int(180))
    );
    assert_eq!(
        eval("rad(180)").unwrap(),
        FormulaValue::Number(NumberValue::Float(std::f64::consts::PI))
    );
    assert_eq!(
        eval("hypot(3, 4)").unwrap(),
        FormulaValue::Number(NumberValue::Int(5))
    );
}

#[test]
fn test_statistical_and_interpolation_functions() {
    assert_eq!(
        eval("sum(1, 2, 3, 4)").unwrap(),
        FormulaValue::Number(NumberValue::Int(10))
    );
    assert_eq!(
        eval("avg(2, 4, 6)").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    );
    assert_eq!(
        eval("mean(2, 4, 6)").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    );
    assert_eq!(
        eval("median(1, 5, 2)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("median(1, 2, 3, 4)").unwrap(),
        FormulaValue::Number(NumberValue::Float(2.5))
    );

    assert_eq!(
        eval("lerp(0, 100, 0.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(50))
    );
    assert_eq!(
        eval("map(5, 0, 10, 0, 100)").unwrap(),
        FormulaValue::Number(NumberValue::Int(50))
    );
}

#[test]
fn test_conditional_and_discrete_functions() {
    assert_eq!(
        eval("if(true, 10, 20)").unwrap(),
        FormulaValue::Number(NumberValue::Int(10))
    );
    assert_eq!(
        eval("if(false, 10, 20)").unwrap(),
        FormulaValue::Number(NumberValue::Int(20))
    );
    // if short-circuits:
    assert_eq!(
        eval_with(
            "if(x > 0, 100 / x, 0)",
            &[("x", FormulaValue::Number(NumberValue::Int(0)))]
        )
        .unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );

    assert_eq!(
        eval("mod(-1, 5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(4))
    );
    assert_eq!(
        eval("mod(7, 3)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );

    assert_eq!(
        eval("gcd(12, 18, 24)").unwrap(),
        FormulaValue::Number(NumberValue::Int(6))
    );
    assert_eq!(
        eval("lcm(4, 6)").unwrap(),
        FormulaValue::Number(NumberValue::Int(12))
    );

    assert_eq!(eval("between(5, 1, 10)").unwrap(), FormulaValue::Bool(true));
    assert_eq!(
        eval("between(0, 1, 10)").unwrap(),
        FormulaValue::Bool(false)
    );
    assert_eq!(eval("isnan(nan)").unwrap(), FormulaValue::Bool(true));
    assert_eq!(eval("isnan(5)").unwrap(), FormulaValue::Bool(false));
    assert_eq!(eval("isfinite(inf)").unwrap(), FormulaValue::Bool(false));
    assert_eq!(eval("isfinite(5)").unwrap(), FormulaValue::Bool(true));
    assert_eq!(eval("finite(5)").unwrap(), FormulaValue::Bool(true));
}

#[test]
fn test_arity_and_unknown_function_errors() {
    assert!(
        matches!(eval("round()"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-wrong-arity")
    );
    assert!(
        matches!(eval("round(1, 2, 3)"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-wrong-arity")
    );
    assert!(
        matches!(eval("unknown_fn(1)"), Err(FormulaError { ref message, .. }) if message.key == "error-formula-unknown-function")
    );
}

#[test]
fn test_float_functions_and_domain_errors() {
    assert_eq!(
        eval("abs(-9223372036854775808)").unwrap(),
        FormulaValue::Number(NumberValue::Float(9223372036854775808.0))
    );
    assert_eq!(
        eval("sign(3.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("sign(-2.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(-1))
    );
    assert_eq!(
        eval("sign(0.0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );

    assert_eq!(
        eval("min(3.5, 1.2)").unwrap(),
        FormulaValue::Number(NumberValue::Float(1.2))
    );
    assert_eq!(
        eval("max(3.5, 1.2)").unwrap(),
        FormulaValue::Number(NumberValue::Float(3.5))
    );
    assert_eq!(
        eval("clamp(3.5, 0.0, 2.0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("avg(1.0, 2.0, 3.0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("sum(1.5, 2.5)").unwrap(),
        FormulaValue::Number(NumberValue::Float(4.0))
    );

    assert_eq!(
        eval("round(1.555, 2)").unwrap(),
        FormulaValue::Number(NumberValue::Float(1.56))
    );
    assert_eq!(
        eval("floor(1.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("ceil(1.2)").unwrap(),
        FormulaValue::Number(NumberValue::Int(2))
    );
    assert_eq!(
        eval("trunc(1.8)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("frac(1.25)").unwrap(),
        FormulaValue::Number(NumberValue::Float(0.25))
    );

    assert_eq!(
        eval("mod(7.5, 2.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert!(eval("mod(5, 0)").is_err());
    assert!(eval("sqrt(-1)").is_err());
    assert!(eval("asin(2)").is_err());
    assert!(eval("acos(2)").is_err());

    assert_eq!(
        eval("sinh(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(0))
    );
    assert_eq!(
        eval("cosh(0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(1))
    );
    assert_eq!(
        eval("lerp(0.0, 10.0, 0.5)").unwrap(),
        FormulaValue::Number(NumberValue::Int(5))
    );
    assert_eq!(
        eval("map(5.0, 0.0, 10.0, 0.0, 100.0)").unwrap(),
        FormulaValue::Number(NumberValue::Int(50))
    );
}
