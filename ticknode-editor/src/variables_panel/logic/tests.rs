use super::*;
use ticknode_shared::{DataType, DataValue, NumberValue, VariableKey};

fn var(key: VariableKey, data_type: DataType, value: DataValue) -> Variable {
    Variable {
        key,
        data_type,
        value,
    }
}

#[test]
fn test_sorting_case_insensitive() {
    let sheet_id = SheetId::generate();
    let types = TypeRegistry::new();
    let vars = vec![
        var(
            VariableKey::sheet(sheet_id, "beta"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(1)),
        ),
        var(
            VariableKey::sheet(sheet_id, "Alpha"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(2)),
        ),
        var(
            VariableKey::sheet(sheet_id, "alpha_2"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(3)),
        ),
        var(
            VariableKey::sheet(sheet_id, "CHARLIE"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(4)),
        ),
    ];

    let groups = group_and_filter_variables(&vars, Some(sheet_id), "", &types);
    let names: Vec<&str> = groups
        .sheet
        .iter()
        .map(|i| i.variable.key.name.as_str())
        .collect();
    assert_eq!(names, vec!["Alpha", "alpha_2", "beta", "CHARLIE"]);
}

#[test]
fn test_shadowing() {
    let sheet_1 = SheetId::generate();
    let sheet_2 = SheetId::generate();
    let types = TypeRegistry::new();

    let vars = vec![
        var(
            VariableKey::sheet(sheet_1, "msg"),
            DataType::String,
            DataValue::String("local".into()),
        ),
        var(
            VariableKey::global("msg"),
            DataType::String,
            DataValue::String("global".into()),
        ),
        var(
            VariableKey::persistent("msg_persistent"),
            DataType::String,
            DataValue::String("pers".into()),
        ),
        var(
            VariableKey::sheet(sheet_1, "msg_persistent"),
            DataType::String,
            DataValue::String("pers_local".into()),
        ),
        var(
            VariableKey::global("unshadowed"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(10)),
        ),
    ];

    // On sheet_1: msg and msg_persistent are shadowed in global/persistent
    let g1 = group_and_filter_variables(&vars, Some(sheet_1), "", &types);
    let global_msg = g1
        .global
        .iter()
        .find(|i| i.variable.key.name == "msg")
        .unwrap();
    assert!(global_msg.shadowed);

    let persistent_msg = g1
        .persistent
        .iter()
        .find(|i| i.variable.key.name == "msg_persistent")
        .unwrap();
    assert!(persistent_msg.shadowed);

    let unshadowed = g1
        .global
        .iter()
        .find(|i| i.variable.key.name == "unshadowed")
        .unwrap();
    assert!(!unshadowed.shadowed);

    // On sheet_2: not shadowed because sheet_1's sheet variable doesn't apply to sheet_2
    let g2 = group_and_filter_variables(&vars, Some(sheet_2), "", &types);
    assert!(g2.sheet.is_empty());
    let global_msg_s2 = g2
        .global
        .iter()
        .find(|i| i.variable.key.name == "msg")
        .unwrap();
    assert!(!global_msg_s2.shadowed);

    // On home page (sheet: None): no sheet variables and no shadowing
    let gh = group_and_filter_variables(&vars, None, "", &types);
    assert!(gh.sheet.is_empty());
    let global_msg_home = gh
        .global
        .iter()
        .find(|i| i.variable.key.name == "msg")
        .unwrap();
    assert!(!global_msg_home.shadowed);
}

#[test]
fn test_home_page_excludes_sheet_variables() {
    let sheet_id = SheetId::generate();
    let types = TypeRegistry::new();
    let vars = vec![
        var(
            VariableKey::sheet(sheet_id, "local_var"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(1)),
        ),
        var(
            VariableKey::global("global_var"),
            DataType::String,
            DataValue::String("hello".into()),
        ),
        var(
            VariableKey::persistent("pers_var"),
            DataType::Bool,
            DataValue::Bool(true),
        ),
    ];

    let groups = group_and_filter_variables(&vars, None, "", &types);
    assert!(groups.sheet.is_empty());
    assert_eq!(groups.global.len(), 1);
    assert_eq!(groups.persistent.len(), 1);
    assert_eq!(groups.total_count(), 2);
}

#[test]
fn test_query_filtering() {
    let sheet_id = SheetId::generate();
    let types = TypeRegistry::new();
    let vars = vec![
        var(
            VariableKey::sheet(sheet_id, "temperature"),
            DataType::Number,
            DataValue::Number(NumberValue::Int(23)),
        ),
        var(
            VariableKey::global("welcome_text"),
            DataType::String,
            DataValue::String("Hello world".into()),
        ),
        var(
            VariableKey::persistent("alarm_active"),
            DataType::Bool,
            DataValue::Bool(false),
        ),
    ];

    // Filter by name term
    let g1 = group_and_filter_variables(&vars, Some(sheet_id), "temp", &types);
    assert_eq!(g1.total_count(), 1);
    assert_eq!(g1.sheet[0].variable.key.name, "temperature");

    // Filter by value text
    let g2 = group_and_filter_variables(&vars, Some(sheet_id), "world", &types);
    assert_eq!(g2.total_count(), 1);
    assert_eq!(g2.global[0].variable.key.name, "welcome_text");

    // Filter by data type name
    let g3 = group_and_filter_variables(&vars, Some(sheet_id), "bool", &types);
    assert_eq!(g3.total_count(), 1);
    assert_eq!(g3.persistent[0].variable.key.name, "alarm_active");

    // Filter with no matches
    let g4 = group_and_filter_variables(&vars, Some(sheet_id), "nonexistent", &types);
    assert!(g4.is_empty());
}
