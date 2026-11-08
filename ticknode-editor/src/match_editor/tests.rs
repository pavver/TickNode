use super::*;

fn cond(text: &str) -> Condition {
    MatchCase::parse(&format!("1: {text}")).unwrap().conditions.remove(0)
}

#[test]
fn conditions_follow_input_type() {
    // Number -> string: ">" is invalid, 5 becomes "5"
    let mut c = cond("a > 5");
    fit_condition(&TypeRegistry::new(), &mut c, &DataType::String);
    assert_eq!(c.op, CompareOp::Eq);
    assert_eq!(c.right, Some(Operand::Value(DataValue::String("5".into()))));
    // "=" is valid for bool as well — stays, value becomes true/false
    fit_condition(&TypeRegistry::new(), &mut c, &DataType::Bool);
    assert_eq!((c.op, c.right.clone()), (CompareOp::Eq, Some(Operand::Value(DataValue::Bool(true)))));
    // ">" is invalid for bool — picks first applicable unary operator "= true"
    let mut c = cond("a > 5");
    fit_condition(&TypeRegistry::new(), &mut c, &DataType::Bool);
    assert_eq!((c.op, c.right.clone()), (CompareOp::IsTrue, None));
    // Time: value is converted to the required type
    let mut c = cond("a > b");
    fit_condition(&TypeRegistry::new(), &mut c, &DataType::Time);
    assert_eq!(c.right, Some(Operand::Input("b".into())));
    let mut c = cond("a = \"07:30\"");
    fit_condition(&TypeRegistry::new(), &mut c, &DataType::Time);
    assert!(matches!(c.right, Some(Operand::Value(DataValue::Time(_)))));
}

#[test]
fn free_names() {
    let i = |n: &str| MatchInput { pin: 1, name: n.into(), value_type: DataType::Any };
    assert_eq!(free_name(&[i("a"), i("c")]), "b");
}

#[test]
fn new_conditions_reuse_the_first_input_or_create_one() {
    let env = Env { types: vec![], reg: TypeRegistry::new() };
    let mut state = MatchState { inputs: vec![], cases: vec![] };
    let first = new_condition(&mut state, &env, Join::Or);
    assert_eq!(state.inputs[0], MatchInput { pin: 1, name: "a".into(), value_type: DataType::Any });
    assert_eq!(first.left, Operand::Input("a".into()));
    assert_eq!(first.join, Join::Or);
    assert_eq!(first.op, CompareOp::Eq);
    assert_eq!(first.right, Some(Operand::Value(DataValue::Number(NumberValue::Int(0)))));
    let env = Env { types: vec![("a".into(), DataType::Bool)], reg: TypeRegistry::new() };
    let second = new_condition(&mut state, &env, Join::And);
    assert_eq!(state.inputs.len(), 1);
    assert_eq!(second.op, CompareOp::IsTrue);
    assert_eq!(second.right, None);
    assert_eq!(type_of(&env.types, &Operand::Value(DataValue::Bool(false))), DataType::Bool);
}

#[test]
fn named_enum_conditions_keep_valid_members_and_default_invalid_ones() {
    let reg = TypeRegistry::from_definitions(vec![TypeDefinition::new_enum("test.mode", "Mode", &[("off", "Off"), ("on", "On")])]);
    let ty = DataType::Named("test.mode".into());
    let mut valid = cond("a = \"on\"");
    fit_condition(&reg, &mut valid, &ty);
    assert_eq!(valid.right, Some(Operand::Value(DataValue::String("on".into()))));
    let mut invalid = cond("a = \"unknown\"");
    fit_condition(&reg, &mut invalid, &ty);
    assert_eq!(invalid.right, Some(Operand::Value(DataValue::String("off".into()))));
    let mut absent = cond("a = \"on\"");
    absent.right = None;
    fit_condition(&reg, &mut absent, &ty);
    assert_eq!(absent.right, Some(Operand::Value(DataValue::String("off".into()))));
}
