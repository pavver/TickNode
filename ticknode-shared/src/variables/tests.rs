use super::*;

#[test]
fn valid_names_and_limits() {
    assert!(valid_variable_name("foo"));
    assert!(valid_variable_name("foo bar"));
    assert!(valid_variable_name("x_1"));
    assert!(valid_variable_name("temp_target"));
    assert!(!valid_variable_name(""));
    assert!(!valid_variable_name("  foo"));
    assert!(!valid_variable_name("foo  "));
    assert!(!valid_variable_name("foo\nbar"));
    assert!(!valid_variable_name("foo\tbar"));
    assert!(!valid_variable_name(&"a".repeat(101)));
    assert!(valid_variable_name(&"a".repeat(100)));
}

#[test]
fn variable_keys_and_validity() {
    let sheet_id = SheetId::generate();
    let sk = VariableKey::sheet(sheet_id, "foo");
    assert!(sk.is_valid());
    let gk = VariableKey::global("bar");
    assert!(gk.is_valid());
    let pk = VariableKey::persistent("baz");
    assert!(pk.is_valid());

    let invalid_sheet = VariableKey {
        scope: VariableScope::Sheet,
        sheet: None,
        name: "foo".into(),
    };
    assert!(!invalid_sheet.is_valid());

    let invalid_global = VariableKey {
        scope: VariableScope::Global,
        sheet: Some(sheet_id),
        name: "bar".into(),
    };
    assert!(!invalid_global.is_valid());
}

#[test]
fn value_size_limits() {
    assert!(valid_variable_value(&DataValue::Number(42.into())));
    let big_string = "x".repeat(MAX_VARIABLE_VALUE_SIZE + 10);
    assert!(!valid_variable_value(&DataValue::String(big_string)));
}

#[test]
fn postcard_roundtrip() {
    let sheet_id = SheetId::generate();
    let var = Variable {
        key: VariableKey::sheet(sheet_id, "test_var"),
        data_type: DataType::Number,
        value: DataValue::Number(123.into()),
    };
    let encoded = postcard::to_allocvec(&var).unwrap();
    let decoded: Variable = postcard::from_bytes(&encoded).unwrap();
    assert_eq!(var, decoded);
}
