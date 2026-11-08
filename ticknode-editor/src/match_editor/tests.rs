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
