use super::*;
use super::CompareOp::*;
use crate::NumberValue;
use crate::ObjectField;
use std::collections::BTreeMap;

fn num(v: i64) -> Option<DataValue> {
    Some(DataValue::Number(NumberValue::Int(v)))
}

#[test]
fn text_round_trip() {
    let case = MatchCase::parse("3: a > 10 and b != \"on \\\"x\\\"\" or c <= -2.5 or s contains \"x\" and l is_empty").unwrap();
    assert_eq!(case.pin, 3);
    assert_eq!(case.conditions.len(), 5);
    assert_eq!(case.conditions[2].join, Join::Or);
    assert_eq!(case.conditions[4].right, None);
    assert_eq!(MatchCase::parse(&case.to_text()).unwrap(), case);
    assert_eq!(case.display(), "a > 10 and b ≠ \"on \\\"x\\\"\" or c ≤ -2.5 or s contains \"x\" and l is empty");
    assert_eq!(MatchCase::parse("7:").unwrap().display(), "always");
    assert_eq!(MatchCase::parse("1: t ≥ 5 && v == true").unwrap().to_text(), "1: t >= 5 and v = true");
    assert_eq!(MatchCase::parse("1: a = b").unwrap().inputs().collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn parse_errors() {
    assert!(MatchCase::parse("a > 1").is_err());
    assert!(MatchCase::parse("1: a >").is_err());
    assert!(MatchCase::parse("1: a > 1 b < 2").is_err());
    assert!(MatchCase::parse("1: a ! 1").is_err());
    assert!(MatchCase::parse("1: a frobnicate 1").is_err());
    assert!(MatchCase::parse("1: contains > 1").is_err(), "an operator cannot be an input name");
    assert!(MatchInput::parse("1:a").is_err());
    assert!(MatchInput::parse("1:2a:number").is_err());
    assert!(MatchInput::parse("1:a:weird").is_err());
    assert_eq!(MatchCase::parse("a > 1"), Err(ConditionError::MissingPin));
    assert_eq!(MatchCase::parse("1: a >"), Err(ConditionError::Expected(Expected::Operand, None)));
    assert_eq!(MatchCase::parse("1: a > 1 b"), Err(ConditionError::Expected(Expected::Join, Some("b".into()))));
    assert_eq!(MatchCase::parse("1: a frobnicate 1"), Err(ConditionError::UnknownOperator("frobnicate".into())));
    assert_eq!(MatchCase::parse("1: a = \"x"), Err(ConditionError::UnclosedQuote));
    assert_eq!(MatchInput::parse("1:a:weird"), Err(ConditionError::UnknownType("weird".into())));
    let m = MatchCase::parse("1: a >").unwrap_err().ui_message();
    assert_eq!(m.key, "error-condition-expected-operand");
    assert_eq!(m.fallback, "expected an operand, not end of text");
    assert_eq!(m.args, vec![("end".to_string(), "true".to_string()), ("found".to_string(), String::new())]);
    let input = MatchInput::parse("4:temp:list").unwrap();
    assert_eq!(input.value_type, DataType::List(Box::new(DataType::Any)));
    assert_eq!(input.to_text(), "4:temp:list");
}

#[test]
fn and_binds_tighter_than_or() {
    let case = MatchCase::parse("1: a > 1 and b > 1 or c > 1").unwrap();
    let eval = |a, b, c| case.matches(|n| match n { "a" => num(a), "b" => num(b), "c" => num(c), _ => None });
    assert!(eval(2, 2, 0));
    assert!(!eval(2, 0, 0));
    assert!(eval(0, 0, 2));
}

#[test]
fn typed_comparisons() {
    let list = DataValue::List { element_type: DataType::Number, elements: vec![num(1).unwrap(), num(5).unwrap()] };
    let mut fields = BTreeMap::new();
    fields.insert("temp".to_string(), ObjectField { name: "temp".into(), description: String::new(), value: num(20).unwrap() });
    let object = DataValue::Object { fields, type_name: None };
    let holds = |text: &str| MatchCase::parse(&format!("1: {text}")).unwrap().matches(|n| match n {
        "s" => Some(DataValue::String("living room".into())),
        "n" => num(5),
        "m" => Some(DataValue::Number(NumberValue::Float(5.0))),
        "t" => Some(DataValue::Time(crate::text::parse_time("07:30").unwrap())),
        "d" => Some(DataValue::Duration(std::time::Duration::from_secs(90))),
        "l" => Some(list.clone()),
        "o" => Some(object.clone()),
        "f" => Some(DataValue::Bool(false)),
        _ => None,
    });
    // Numbers and two inputs
    assert!(holds("n >= 5") && holds("n = m") && holds("n != \"6\"") && !holds("n < \"5\""));
    // Strings
    assert!(holds("s contains \"room\"") && holds("s starts_with \"living\"") && holds("s ends_with \"room\""));
    assert!(holds("s not_contains \"kitchen\"") && holds("s not_empty") && holds("s len_gt 5"));
    // Time, duration — string constant is coerced to the input type
    assert!(holds("t < \"08:00\"") && holds("t = \"07:30\"") && !holds("t > \"07:30:01\""));
    assert!(holds("d > \"1m\"") && holds("d = 90") && holds("d < \"2m\""));
    // Lists, objects, booleans
    assert!(holds("l contains 5") && holds("l not_contains 7") && holds("l len_eq 2") && !holds("l is_empty"));
    assert!(holds("o has_field \"temp\"") && holds("o no_field \"hum\""));
    assert!(holds("f is_false") && !holds("f is_true"));
    // Missing value
    assert!(!holds("missing = 0") && !holds("missing != 0") && holds("missing no_value") && holds("n has_value"));
    // Operator inapplicable to type — evaluates to false
    assert!(!holds("n starts_with \"5\"") && !holds("f contains 1"));
}

#[test]
fn ops_follow_type() {
    assert!(ops_for(&DataType::String).contains(&Contains));
    assert!(!ops_for(&DataType::Number).contains(&Contains));
    assert_eq!(Contains.right_type(&DataType::List(Box::new(DataType::Number))), Some(DataType::Number));
    assert_eq!(LenGt.right_type(&DataType::String), Some(DataType::Number));
    assert_eq!(IsEmpty.right_type(&DataType::String), None);
    assert_eq!(Lt.label_for(&DataType::Time), "< earlier");
}

#[test]
fn underscore_input_names_parse_and_resolve() {
    let input = MatchInput::parse("7:_sensor_1:number").unwrap();
    assert_eq!(input.name, "_sensor_1");
    let case = MatchCase::parse("1:_sensor_1 = 12").unwrap();
    assert!(case.matches(|name| if name == input.name { num(12) } else { None }));
    assert!(!case.matches(|name| if name == input.name { num(13) } else { None }));
}

#[test]
fn inclusive_comparisons_accept_equality_and_reject_the_other_side() {
    for (op, a, b, expected) in [
        (Le, 4, 5, true), (Le, 5, 5, true), (Le, 6, 5, false),
        (Ge, 4, 5, false), (Ge, 5, 5, true), (Ge, 6, 5, true),
    ] {
        let case = MatchCase::parse(&format!("1:a {} {b}", op.text())).unwrap();
        assert_eq!(case.matches(|_| num(a)), expected, "{a} {op:?} {b}");
    }
}

#[test]
fn missing_right_operand_never_satisfies_a_binary_condition() {
    let mut case = MatchCase::parse("1:a != missing").unwrap();
    let values = |name: &str| if name == "a" { num(5) } else { None };
    assert!(!case.matches(values));
    case.conditions[0].right = None;
    assert!(!case.matches(values));
}

#[test]
fn empty_and_single_element_collections_have_opposite_emptiness() {
    for (empty, single) in [
        (DataValue::from(""), DataValue::from("я")),
        (
            DataValue::List { element_type: DataType::Number, elements: vec![] },
            DataValue::List { element_type: DataType::Number, elements: vec![1i64.into()] },
        ),
        (
            DataValue::Object { fields: BTreeMap::new(), type_name: None },
            DataValue::Object {
                fields: [("x".into(), ObjectField { name: "x".into(), description: String::new(), value: 1i64.into() })].into(),
                type_name: None,
            },
        ),
    ] {
        for (text, empty_matches, single_matches) in [("1:a is_empty", true, false), ("1:a not_empty", false, true)] {
            let case = MatchCase::parse(text).unwrap();
            assert_eq!(case.matches(|_| Some(empty.clone())), empty_matches, "{text}: {empty:?}");
            assert_eq!(case.matches(|_| Some(single.clone())), single_matches, "{text}: {single:?}");
        }
    }
}

#[test]
fn structural_equality_compares_collection_contents() {
    let list = |n| DataValue::List { element_type: DataType::Number, elements: vec![DataValue::from(n)] };
    let case = MatchCase::parse("1:a = b").unwrap();
    assert!(case.matches(|_| Some(list(1i64))));
    assert!(!case.matches(|name| Some(list(if name == "a" { 1i64 } else { 2i64 }))));
}

#[test]
fn empty_case_matches_without_resolving_inputs() {
    assert!(MatchCase::parse("7:").unwrap().matches(|_| panic!("an empty case has no inputs")));
}

#[test]
fn first_join_is_ignored_even_when_constructed_as_or() {
    let mut case = MatchCase::parse("1:a > 5").unwrap();
    case.conditions[0].join = Join::Or;
    assert!(!case.matches(|_| num(4)));
    assert!(case.matches(|_| num(6)));
}

#[test]
fn compact_negative_literal_and_false_are_evaluated() {
    let case = MatchCase::parse("1:a=-12 and flag=false").unwrap();
    assert_eq!(case.to_text(), "1: a = -12 and flag = false");
    assert!(case.matches(|name| if name == "a" { num(-12) } else { Some(DataValue::Bool(false)) }));
    assert!(!case.matches(|name| if name == "a" { num(-12) } else { Some(DataValue::Bool(true)) }));
    assert_eq!(MatchCase::parse("1:a=-"), Err(ConditionError::UnexpectedChar('-')));
}

#[test]
fn operators_preserve_results_and_lazy_operand_resolution() {
    use std::cell::RefCell;

    for (op, expected) in [
        (Eq, false), (Ne, true), (Lt, true), (Le, true), (Gt, false), (Ge, false),
        (Contains, false), (NotContains, false), (StartsWith, false), (EndsWith, false),
        (IsEmpty, false), (NotEmpty, false), (IsTrue, false), (IsFalse, false),
        (HasField, false), (NoField, false), (LenEq, false), (LenGt, false), (LenLt, false),
        (HasValue, true), (NoValue, false),
    ] {
        let condition = Condition {
            join: Join::And, left: Operand::Input("left".into()), op,
            right: Some(Operand::Input("right".into())),
        };
        let reads = RefCell::new(Vec::new());
        assert_eq!(condition.holds(&|name| {
            reads.borrow_mut().push(name.to_string());
            num(if name == "left" { 1 } else { 2 })
        }), expected, "{op:?}");
        let expected_reads = if op.is_unary() { vec!["left"] } else { vec!["left", "right"] };
        assert_eq!(*reads.borrow(), expected_reads, "{op:?}");

        reads.borrow_mut().clear();
        assert_eq!(condition.holds(&|name| {
            reads.borrow_mut().push(name.to_string());
            None
        }), op == NoValue, "missing left: {op:?}");
        assert_eq!(*reads.borrow(), ["left"], "missing left: {op:?}");
    }
}
