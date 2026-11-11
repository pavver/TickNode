use super::*;
use crate::{DataType, DataValue, NumberValue};

#[test]
fn test_variables_derivation() {
    let source = "z and a are plain text {# ignored #}{{ obj.a }}{{ z }}{{ a }}\
        {{ 'fake' }}{% for item in items %}{{ item }}{% endfor %}\
        {% set local = other %}{{ local }}{{ range(2) }}{{ z }}";
    let vars = variables(source).unwrap();
    assert_eq!(vars, vec!["obj", "z", "a", "items", "other"]);
}

#[test]
fn test_variables_with_text_references_and_reserved_names() {
    let source = "{{ text is mapping }}{{ text.power }}{{ obj.text }}{{ power }}\
        {{ text.name }}{{ name }}{{ 'literal' | text }}{{ range(1) }}";
    let vars = variables(source).unwrap();
    assert_eq!(vars, vec!["power", "obj", "name"]);

    let source2 = "{{ power }}{{ text.name }}{{ text }}";
    let vars2 = variables(source2).unwrap();
    assert_eq!(vars2, vec!["power", "name"]);

    let source3 = "{% set text = 'local' %}{{ text }}|{{ x }}";
    let vars3 = variables(source3).unwrap();
    assert_eq!(vars3, vec!["x"]);
}

#[test]
fn test_variables_syntax_error() {
    let err = variables("{% if %}").unwrap_err();
    assert_eq!(err.line, Some(1));
    assert!(err.range.is_some());
}

#[test]
fn test_syntax_errors_with_line_column_and_range() {
    let source = "valid line 1\n{% if %}\nline 3";
    let err = validate(source).unwrap_err();
    assert_eq!(err.line, Some(2));
    assert!(err.column.is_some());
    assert!(err.range.is_some());

    let source2 = "\n\n{% bad tag %}";
    let err2 = validate(source2).unwrap_err();
    assert_eq!(err2.line, Some(3));
}

#[test]
fn test_render_error_with_line() {
    let source = "line 1\nline 2\n{{ 1 // 0 }}";
    let inputs = vec![];
    let err = render(source, &inputs, &|_, _| String::new(), std::sync::Arc::new(|_: &DataValue| String::new())).unwrap_err();
    assert_eq!(err.line, Some(3));
}

#[test]
fn test_render_fuel_exhaustion() {
    let source = "{% for i in range(100000) %}{{ i }}{% endfor %}";
    let inputs = vec![];
    let err = render(source, &inputs, &|_, _| String::new(), std::sync::Arc::new(|_: &DataValue| String::new())).unwrap_err();
    assert!(err.message.contains("fuel"));
}

#[test]
fn test_render_inputs_format_and_runtime_text() {
    let source = "{{ x + 1 }}|{{ text.power }}|{{ (power / 1000) | text }}|{{ text.missing | default('unset') }}";
    let inputs = vec![
        ("x".into(), Some(DataValue::Number(NumberValue::Int(10))), DataType::Number),
        ("power".into(), Some(DataValue::Number(NumberValue::Int(1500))), DataType::Named("quantity.power".into())),
        ("missing".into(), None, DataType::String),
    ];
    let format = |val: &DataValue, ty: &DataType| -> String {
        match (val, ty) {
            (DataValue::Number(NumberValue::Int(n)), DataType::Named(name)) if name == "quantity.power" => {
                format!("{} W", n)
            }
            _ => format!("{val:?}"),
        }
    };
    let runtime_text = |val: &DataValue| -> String {
        match val {
            DataValue::Number(NumberValue::Float(f)) => format!("{f:.1}"),
            DataValue::Number(NumberValue::Int(n)) => format!("{n}"),
            _ => format!("{val:?}"),
        }
    };
    let result = render(source, &inputs, &format, std::sync::Arc::new(runtime_text)).unwrap();
    assert_eq!(result, "11|1500 W|1.5|unset");
}

#[test]
fn test_plain_values() {
    assert!(plain(&DataValue::Number(NumberValue::Int(2))).is_integer());
    assert!(!plain(&DataValue::Number(NumberValue::Float(2.0))).is_integer());
    assert_eq!(plain(&DataValue::Bool(true)).is_true(), true);
    assert_eq!(plain(&DataValue::String("hi".into())).as_str(), Some("hi"));
}
