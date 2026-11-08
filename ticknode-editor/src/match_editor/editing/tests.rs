use super::*;

fn state() -> MatchState {
    MatchState {
        inputs: vec![MatchInput { pin: 1, name: "a".into(), value_type: DataType::Number },
            MatchInput { pin: 2, name: "b".into(), value_type: DataType::Number }],
        cases: vec![MatchCase::parse("4: a > b and b = 5").unwrap()],
    }
}

#[test]
fn renaming_validates_names_and_updates_both_operands_without_changing_pins() {
    let mut state = state();
    assert!(validate_name(&state, 0, "bad name").is_err());
    assert!(validate_name(&state, 0, "b").is_err());
    assert_eq!(validate_name(&state, 0, " a ").unwrap(), "a");
    let name = validate_name(&state, 1, " reading ").unwrap();
    rename_input(&mut state, 1, name);
    assert_eq!(state.inputs[1].pin, 2);
    assert_eq!(state.inputs[1].name, "reading");
    assert_eq!(state.cases[0].to_text(), "4: a > reading and reading = 5");
}

#[test]
fn removing_an_input_drops_its_cases_and_resets_right_operand_references() {
    let mut state = state();
    let env = Env { types: vec![("a".into(), DataType::Number)], reg: TypeRegistry::new() };
    remove_input(&mut state, &env, 1);
    assert_eq!(state.inputs.len(), 1);
    assert_eq!(state.inputs[0].name, "a");
    assert_eq!(state.cases[0].pin, 4);
    assert_eq!(state.cases[0].to_text(), "4: a > 0");
}

#[test]
fn type_changes_adapt_only_conditions_using_the_left_input() {
    let mut state = state();
    let env = Env { types: vec![("a".into(), DataType::Bool)], reg: TypeRegistry::new() };
    set_input_type(&mut state, &env, 0, DataType::Any);
    assert_eq!(state.inputs[0].value_type, DataType::Any);
    assert_eq!(state.cases[0].conditions[0].op, CompareOp::IsTrue);
    assert_eq!(state.cases[0].conditions[0].right, None);
    assert_eq!(state.cases[0].conditions[1].op, CompareOp::Eq);
    set_input_type(&mut state, &env, 1, DataType::String);
    assert_eq!(state.cases[0].conditions[1].right, Some(Operand::Value(DataValue::String("5".into()))));
}
