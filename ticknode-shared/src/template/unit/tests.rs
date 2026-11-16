use super::*;
use crate::{
    DataType, DataValue, FieldDefinition, NumberValue, ObjectField, TypeDefinition, TypeRegistry,
    template::{CompiledTemplate, CompletionInput, RenderEnv, complete, render},
};
use std::collections::BTreeMap;
use std::sync::Arc;

fn power_input() -> Vec<(String, Option<DataValue>, DataType)> {
    vec![(
        "power".to_string(),
        Some(DataValue::Number(NumberValue::Float(1970.0))),
        DataType::Named("quantity.power".to_string()),
    )]
}

#[test]
fn test_unit_quantity_default_rules() {
    let inputs = power_input();
    let res = render(
        "{{ power | unit }}",
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "1.97 kW");
}

#[test]
fn test_unit_explicit_symbol() {
    let inputs = power_input();
    let res = render(
        "{{ power | unit('W') }}",
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "1970 W");
}

#[test]
fn test_unit_explicit_symbol_with_decimals() {
    let inputs = power_input();
    let res = render(
        "{{ power | unit('kW', decimals=1) }}",
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "2.0 kW");
}

#[test]
fn test_unit_computed_expression_with_symbol() {
    let inputs = vec![(
        "p".to_string(),
        Some(DataValue::Number(NumberValue::Float(3940.0))),
        DataType::Named("quantity.power".to_string()),
    )];
    let res = render(
        "{{ (p / 2) | unit('kW') }}",
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "1.97 kW");
}

#[test]
fn test_unit_plain_number_input() {
    let inputs = vec![(
        "num".to_string(),
        Some(DataValue::Number(NumberValue::Float(1234.5))),
        DataType::Number,
    )];
    let res = render(
        "{{ num | unit }}",
        &inputs,
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "1234");
}

#[test]
fn test_unit_struct_field_path() {
    let mut types = TypeRegistry::from_definitions(crate::quantity::builtin_quantities());
    let def = TypeDefinition::new_struct(
        "device.sensor",
        "Sensor",
        vec![FieldDefinition::new(
            "power",
            "Power",
            DataType::Named("quantity.power".to_string()),
        )],
    );
    types.insert_all(&[def]).unwrap();

    let mut fields = BTreeMap::new();
    fields.insert(
        "power".to_string(),
        ObjectField {
            data_type: None,
            value: DataValue::Number(NumberValue::Float(1970.0)),
        },
    );
    let sensor_val = DataValue::Object {
        type_name: Some("device.sensor".to_string()),
        fields,
    };
    let inputs = vec![(
        "sensor".to_string(),
        Some(sensor_val),
        DataType::Named("device.sensor".to_string()),
    )];

    let compiled = CompiledTemplate::new("{{ sensor.power | unit }}").unwrap();
    let res = compiled
        .render_localized(
            &inputs,
            &|v, _| crate::text::format_value(v),
            Arc::new(crate::text::format_value),
            RenderEnv::default(),
            &types,
        )
        .unwrap();
    assert_eq!(res, "1.97 kW");
}

#[test]
fn test_unit_local_set_not_rewritten() {
    let source = "{% set local_p = 5000 %}{{ local_p | unit }}";
    let (rewritten, records) = rewrite_unit_paths(source);
    assert!(!rewritten.contains("__path"));
    assert!(records.is_empty());

    let res = render(
        source,
        &[],
        &|v, _| crate::text::format_value(v),
        Arc::new(crate::text::format_value),
    )
    .unwrap();
    assert_eq!(res, "5000");
}

#[test]
fn test_unit_runtime_error_reports_original_column() {
    let source = "{{ power | unit }} {{ 1 // 0 }}";
    let compiled = CompiledTemplate::new(source).unwrap();
    let inputs = power_input();
    let err = compiled
        .render(
            &inputs,
            &|v, _| crate::text::format_value(v),
            Arc::new(crate::text::format_value),
        )
        .unwrap_err();

    assert_eq!(err.line, Some(1));
    assert!(
        err.column.is_some_and(|col| (20..=26).contains(&col)),
        "Expected error column in 20..=26, got {:?}",
        err.column
    );
}

#[test]
fn test_unit_mismatched_quantity_error() {
    let inputs = power_input();
    let compiled = CompiledTemplate::new("{{ power | unit('m') }}").unwrap();
    let err = compiled
        .render(
            &inputs,
            &|v, _| crate::text::format_value(v),
            Arc::new(crate::text::format_value),
        )
        .unwrap_err();
    assert!(
        err.message
            .contains("unit m does not belong to quantity.power"),
        "Unexpected error message: {}",
        err.message
    );
}

#[test]
fn test_unit_completion_inside_quotes() {
    let inputs = vec![(
        "power".to_string(),
        DataType::Named("quantity.power".to_string()),
    )];
    let source = "{{ power | unit('";
    let caret = source.len();
    let comp = complete(&CompletionInput {
        source,
        caret,
        inputs: &inputs,
        types: &TypeRegistry::new(),
        now: None,
        labels: Some(&crate::quantity::CanonicalLabels),
        variables: &[],
    })
    .unwrap();

    let labels: Vec<_> = comp.items.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"W"),
        "Expected 'W' in completion items: {labels:?}"
    );
    assert!(
        labels.contains(&"kW"),
        "Expected 'kW' in completion items: {labels:?}"
    );
}
