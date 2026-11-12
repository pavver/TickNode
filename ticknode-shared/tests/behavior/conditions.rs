use ticknode_shared::condition::*;
use ticknode_shared::*;

#[test]
fn parser_errors_preserve_log_text_and_translation_arguments() {
    let cases = [
        (
            "x",
            "missing-pin",
            "missing pin number (\"1: a > 0\")",
            vec![],
        ),
        (
            "x:",
            "not-pin-number",
            "\"x\" is not a pin number",
            vec![("text", "x")],
        ),
        (
            "1: a ? 2",
            "unexpected-char",
            "unexpected character \"?\"",
            vec![("char", "?")],
        ),
        (
            "1: a & b",
            "doubled-operator",
            "\"&\" — probably \"&&\"",
            vec![("char", "&")],
        ),
        (
            "1: a = 1.2.3",
            "not-number",
            "\"1.2.3\" is not a number",
            vec![("text", "1.2.3")],
        ),
        ("1: a = \"x\\", "unclosed-quote", "unclosed quote", vec![]),
        (
            "1: a what 2",
            "unknown-operator",
            "\"what\" is an unknown operator",
            vec![("operator", "what")],
        ),
        (
            "1: a",
            "expected-operator",
            "expected an operator, not end of text",
            vec![("end", "true"), ("found", "")],
        ),
        (
            "1: a =",
            "expected-operand",
            "expected an operand, not end of text",
            vec![("end", "true"), ("found", "")],
        ),
        (
            "1: a = 1 true",
            "expected-join",
            "expected \"and\" / \"or\", not «true»",
            vec![("end", "false"), ("found", "true")],
        ),
        (
            "1: a = >",
            "expected-operand",
            "expected an operand, not «>»",
            vec![("end", "false"), ("found", ">")],
        ),
        (
            "1: a and",
            "expected-operator",
            "expected an operator, not «and»",
            vec![("end", "false"), ("found", "and")],
        ),
    ];
    for (input, key, fallback, args) in cases {
        let error = MatchCase::parse(input).unwrap_err();
        assert_eq!(error.to_string(), fallback, "{input}");
        let message = error.ui_message();
        assert_eq!(message.key, format!("error-condition-{key}"));
        assert_eq!(message.fallback, fallback);
        assert_eq!(
            message.args,
            args.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn invalid_input_errors_are_localizable() {
    for (input, key, fallback, arg, value) in [
        (
            "1:a",
            "input-format",
            "\"1:a\": an input is written as \"pin:name:type\"",
            "text",
            "1:a",
        ),
        (
            "1:2a:number",
            "invalid-input-name",
            "\"2a\" is not a valid input name",
            "name",
            "2a",
        ),
        (
            "1:a:unknown",
            "unknown-type",
            "\"unknown\" is an unknown type",
            "type",
            "unknown",
        ),
    ] {
        let error = MatchInput::parse(input).unwrap_err();
        assert_eq!(error.to_string(), fallback);
        assert_eq!(
            error.ui_message(),
            UiMessage::new(format!("error-condition-{key}"), fallback).arg(arg, value)
        );
    }
}

#[test]
fn operators_keep_their_storage_and_display_forms() {
    let forms = [
        ("=", "="),
        ("!=", "≠"),
        ("<", "<"),
        ("<=", "≤"),
        (">", ">"),
        (">=", "≥"),
        ("contains", "contains"),
        ("not_contains", "not contains"),
        ("starts_with", "starts with"),
        ("ends_with", "ends with"),
        ("is_empty", "is empty"),
        ("not_empty", "is not empty"),
        ("is_true", "= true"),
        ("is_false", "= false"),
        ("has_field", "has field"),
        ("no_field", "no field"),
        ("len_eq", "length ="),
        ("len_gt", "length >"),
        ("len_lt", "length <"),
        ("has_value", "has value"),
        ("no_value", "no value"),
    ];
    for (op, (text, symbol)) in CompareOp::ALL.into_iter().zip(forms) {
        assert_eq!(op.text(), text);
        assert_eq!(op.symbol(), symbol);
        assert_eq!(CompareOp::parse(text), Some(op));
        assert_eq!(op.label_for(&DataType::Number), symbol);
    }
    for ty in [DataType::Time, DataType::DateTime] {
        for (op, label) in [
            (CompareOp::Lt, "< earlier"),
            (CompareOp::Le, "≤ not later"),
            (CompareOp::Gt, "> later"),
            (CompareOp::Ge, "≥ not earlier"),
        ] {
            assert_eq!(op.label_for(&ty), label);
        }
    }
    assert_eq!(CompareOp::Lt.label_for(&DataType::Duration), "< shorter");
    assert_eq!(CompareOp::Gt.label_for(&DataType::Duration), "> longer");
}

#[test]
fn rename_only_changes_matching_input_operands() {
    let mut case = MatchCase::parse("2: old = other or text = \"old\"").unwrap();
    case.rename_input("old", "new");
    case.rename_input("other", "source");
    assert_eq!(case.to_text(), "2: new = source or text = \"old\"");
}

#[test]
fn collection_comparisons_preserve_type_mismatches_and_unicode_lengths() {
    let list = DataValue::List {
        element_type: DataType::Number,
        elements: vec![DataValue::from(2i64)],
    };
    let object = DataValue::Object {
        fields: [(
            "room".into(),
            ObjectField {
                name: "room".into(),
                description: String::new(),
                value: DataValue::Bool(true),
            },
        )]
        .into(),
        type_name: None,
    };
    for (value, expression, expected) in [
        (DataValue::from("кімната"), "a len_eq 7", true),
        (DataValue::from("кімната"), "a len_lt 7", false),
        (DataValue::from("кімната"), "a len_gt 6", true),
        (DataValue::from("кімната"), "a len_eq \"bad\"", false),
        (list.clone(), "a contains \"2\"", true),
        (list, "a contains 3", false),
        (object.clone(), "a contains \"room\"", true),
        (object, "a no_field \"room\"", false),
        (DataValue::Bool(false), "a no_field \"room\"", false),
        (DataValue::Bool(false), "a not_contains 1", false),
        (DataValue::from(1i64), "a len_lt 3", false),
        (DataValue::from(1i64), "a < true", false),
    ] {
        let case = MatchCase::parse(&format!("1: {expression}")).unwrap();
        assert_eq!(
            case.matches(|_| Some(value.clone())),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn operator_operands_follow_the_selected_type() {
    use CompareOp::*;
    for (op, left, expected) in [
        (
            Contains,
            DataType::List(Box::new(DataType::Number)),
            Some(DataType::Number),
        ),
        (Contains, DataType::Any, Some(DataType::Any)),
        (NotContains, DataType::String, Some(DataType::String)),
        (StartsWith, DataType::String, Some(DataType::String)),
        (EndsWith, DataType::String, Some(DataType::String)),
        (HasField, DataType::Object, Some(DataType::String)),
        (NoField, DataType::Object, Some(DataType::String)),
        (LenEq, DataType::String, Some(DataType::Number)),
        (LenGt, DataType::String, Some(DataType::Number)),
        (LenLt, DataType::String, Some(DataType::Number)),
        (Eq, DataType::Time, Some(DataType::Time)),
        (IsTrue, DataType::Bool, None),
    ] {
        assert_eq!(op.right_type(&left), expected);
    }
    for (ty, expected) in [
        (
            DataType::Number,
            vec![Eq, Ne, Lt, Le, Gt, Ge, HasValue, NoValue],
        ),
        (
            DataType::Bool,
            vec![IsTrue, IsFalse, Eq, Ne, HasValue, NoValue],
        ),
        (DataType::Color, vec![Eq, Ne, HasValue, NoValue]),
        (
            DataType::String,
            vec![
                Eq,
                Ne,
                Contains,
                NotContains,
                StartsWith,
                EndsWith,
                IsEmpty,
                NotEmpty,
                LenEq,
                LenGt,
                LenLt,
                HasValue,
                NoValue,
            ],
        ),
        (
            DataType::List(Box::new(DataType::Number)),
            vec![
                Contains,
                NotContains,
                IsEmpty,
                NotEmpty,
                LenEq,
                LenGt,
                LenLt,
                Eq,
                Ne,
                HasValue,
                NoValue,
            ],
        ),
        (
            DataType::Object,
            vec![
                HasField, NoField, IsEmpty, NotEmpty, Eq, Ne, HasValue, NoValue,
            ],
        ),
        (
            DataType::Named("t.mode".into()),
            vec![Eq, Ne, HasValue, NoValue],
        ),
        (
            DataType::Any,
            vec![
                Eq,
                Ne,
                Lt,
                Le,
                Gt,
                Ge,
                Contains,
                NotContains,
                IsEmpty,
                NotEmpty,
                IsTrue,
                IsFalse,
                HasValue,
                NoValue,
            ],
        ),
    ] {
        assert_eq!(ops_for(&ty), expected);
    }
}
