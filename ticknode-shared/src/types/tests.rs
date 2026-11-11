use super::*;
use crate::condition::{CompareOp, MatchCase, MatchInput, ops_in};
use crate::{DataValue, NumberValue, input_value_editable, input_value_fits, parse_data_type, text};

fn registry() -> TypeRegistry {
    let mut r = TypeRegistry::new();
    r.insert_all(&[
        TypeDefinition::new_struct("t.point", "Point", vec![
            FieldDefinition::new("x", "X", DataType::Number),
            FieldDefinition::new("label", "Label", DataType::String).optional(),
        ]),
        TypeDefinition::new_enum("t.mode", "Mode", &[("auto", "Auto"), ("manual", "Manual")]),
    ]).unwrap();
    r
}

fn named(n: &str) -> DataType {
    DataType::Named(n.into())
}

#[test]
fn primitive_conversions_and_failures() {
    use DataType::*;
    let r = registry();
    let cases = [
        (String, Number, DataValue::from("21.5"), Some(DataValue::from(21.5))),
        (String, Number, DataValue::from("unavailable"), None),
        (String, Number, DataValue::from("NaN"), None),
        (String, Number, DataValue::from("inf"), None),
        (Number, String, DataValue::from(42i64), Some(DataValue::from("42"))),
        (Number, String, DataValue::from(21.5), Some(DataValue::from("21.5"))),
        (Bool, String, DataValue::Bool(false), Some(DataValue::from("false"))),
        (Number, Bool, DataValue::from(-2i64), Some(DataValue::Bool(true))),
        (Number, Bool, DataValue::from(0i64), Some(DataValue::Bool(false))),
        (Bool, Number, DataValue::Bool(true), Some(DataValue::from(1i64))),
        (Bool, Number, DataValue::Bool(false), Some(DataValue::from(0i64))),
        (String, named("t.mode"), DataValue::from("auto"), Some(DataValue::from("auto"))),
        (String, named("t.mode"), DataValue::from("Auto"), None),
        (String, named("t.mode"), DataValue::from("other"), None),
    ];
    for (src, dst, value, expected) in cases {
        assert_eq!(r.convertible(&src, &dst).unwrap().apply(value.clone()), expected, "{src:?} -> {dst:?}: {value:?}");
    }
    for (yes, no) in [("on", "off"), ("true", "false"), ("yes", "no"), ("1", "0"), ("open", "closed"), ("home", "not_home"), ("playing", "paused"), ("locked", "unlocked"), ("detected", "clear"), ("active", "inactive"), ("occupied", "unoccupied"), ("connected", "disconnected"), (" ON ", " OFF ")] {
        let conv = r.convertible(&String, &Bool).unwrap();
        assert_eq!(conv.apply(DataValue::from(yes)), Some(DataValue::Bool(true)));
        assert_eq!(conv.apply(DataValue::from(no)), Some(DataValue::Bool(false)));
    }
    for unknown in ["unknown", "unavailable", "", "2"] {
        assert_eq!(r.convertible(&String, &Bool).unwrap().apply(DataValue::from(unknown)), None);
    }
    for (dst, input) in [(Time, "07:30"), (DateTime, "2026-10-31 12:45"), (Duration, "1h 30m"), (Color, "#12345678")] {
        let value = text::parse_value(input, &dst).unwrap();
        assert_eq!(r.convertible(&String, &dst).unwrap().apply(DataValue::from(input)), Some(value.clone()));
        assert_eq!(r.convertible(&dst, &String).unwrap().apply(value.clone()), Some(DataValue::from(text::format_value(&value))));
        assert_eq!(r.convertible(&String, &dst).unwrap().apply(DataValue::from("invalid")), None);
    }
    for (src, dst) in [(Number, Number), (Any, Number), (named("t.mode"), String), (Object, named("t.point")), (String, named("t.point")), (Number, Time), (named("t.mode"), Number)] {
        assert_eq!(r.convertible(&src, &dst), None, "{src:?} -> {dst:?}");
    }
}

#[test]
fn list_conversion_is_elementwise_and_atomic() {
    let r = registry();
    let list = |t| DataType::List(Box::new(t));
    let conv = r.convertible(&list(DataType::String), &list(DataType::Number)).unwrap();
    let value = |texts: &[&str]| DataValue::List { element_type: DataType::String, elements: texts.iter().map(|s| DataValue::from(*s)).collect() };
    assert_eq!(conv.apply(value(&["1", "2.5"])), Some(DataValue::List { element_type: DataType::Number, elements: vec![DataValue::from(1i64), DataValue::from(2.5)] }));
    assert_eq!(conv.apply(value(&["1", "bad"])), None);
    assert_eq!(conv.apply(value(&[])), Some(DataValue::List { element_type: DataType::Number, elements: vec![] }));
    assert!(r.convertible(&list(DataType::Object), &list(DataType::Number)).is_none());
}

#[test]
fn compatibility_rules() {
    let r = registry();
    assert!(r.compatible(&named("t.point"), &named("t.point")));
    assert!(!r.compatible(&named("t.point"), &named("t.mode")));
    assert!(r.compatible(&named("t.point"), &DataType::Any));
    assert!(r.compatible(&DataType::Any, &named("t.mode")));
    assert!(r.compatible(&named("t.point"), &DataType::Object));
    assert!(!r.compatible(&DataType::Object, &named("t.point")));
    assert!(r.compatible(&named("t.mode"), &DataType::String));
    assert!(!r.compatible(&DataType::String, &named("t.mode")));
    assert!(!r.compatible(&named("t.mode"), &DataType::Object));
    assert!(!r.compatible(&named("t.point"), &DataType::String));
    assert!(r.compatible(&DataType::List(Box::new(named("t.point"))), &DataType::List(Box::new(DataType::Object))));
    assert!(!r.compatible(&named("unknown.type"), &DataType::Object));
}

#[test]
fn registration_is_validated_atomically() {
    let mut r = registry();
    assert!(r.insert_all(&[TypeDefinition::new_enum("t.mode", "Dup", &[("a", "A")])]).is_err());
    let bad = TypeDefinition::new_struct("t.bad", "Bad", vec![FieldDefinition::new("p", "P", named("no.such"))]);
    let ok = TypeDefinition::new_enum("t.fine", "Fine", &[("a", "A")]);
    assert!(r.insert_all(&[ok, bad]).is_err());
    assert!(r.get("t.fine").is_none(), "a batch registers entirely or not at all");
    assert!(r.insert_all(&[TypeDefinition::new_enum("t.empty", "E", &[])]).is_err());
    assert!(r.insert_all(&[TypeDefinition::new_enum("NoDot", "E", &[("a", "A")])]).is_err());
    // A type may reference another type from the same batch
    let a = TypeDefinition::new_struct("t.a", "A", vec![FieldDefinition::new("b", "B", named("t.b"))]);
    let b = TypeDefinition::new_enum("t.b", "B", &[("x", "X")]);
    r.insert_all(&[a, b]).unwrap();
}

#[test]
fn type_names_in_text() {
    assert_eq!(parse_data_type("t.point"), Some(named("t.point")));
    assert_eq!(parse_data_type("list<t.point>"), Some(DataType::List(Box::new(named("t.point")))));
    assert_eq!(parse_data_type("point"), None);
    assert_eq!(crate::data_type_name(&DataType::List(Box::new(named("t.point")))), "list<t.point>");
}

#[test]
fn enum_text_and_input_values() {
    let r = registry();
    let mode = named("t.mode");
    assert_eq!(text::parse_value_in(&r, "auto", &mode), Ok(DataValue::String("auto".into())));
    assert_eq!(text::parse_value_in(&r, "Manual", &mode), Ok(DataValue::String("manual".into())));
    assert!(text::parse_value_in(&r, "other", &mode).is_err());
    assert!(text::parse_value_in(&r, "1", &named("t.point")).is_err());
    assert!(input_value_editable(&r, &mode));
    assert!(!input_value_editable(&r, &named("t.point")));
    assert!(input_value_fits(&r, &mode, &DataValue::String("manual".into())));
    assert!(!input_value_fits(&r, &mode, &DataValue::String("nope".into())));
    assert!(!input_value_fits(&r, &mode, &DataValue::Number(NumberValue::Int(1))));
}

#[test]
fn tagged_object_has_named_type_and_survives_postcard() {
    let mut fields = BTreeMap::new();
    fields.insert("x".to_string(), crate::ObjectField { name: "x".into(), description: String::new(), value: DataValue::Number(NumberValue::Int(1)) });
    let v = DataValue::Object { fields, type_name: Some("t.point".into()) };
    assert_eq!(v.get_type(), named("t.point"));
    let back: DataValue = postcard::from_bytes(&postcard::to_allocvec(&v).unwrap()).unwrap();
    assert_eq!(back, v);
    // Legacy graph JSON (without type_name) deserializes as a plain object
    let old: DataValue = serde_json::from_str(r#"{"Object":{"fields":{}}}"#).unwrap();
    assert_eq!(old.get_type(), DataType::Object);
}

#[test]
fn match_treats_struct_as_object_and_enum_as_string() {
    let r = registry();
    assert!(ops_in(&r, &named("t.point")).contains(&CompareOp::HasField));
    assert!(!ops_in(&r, &named("t.mode")).contains(&CompareOp::HasField));
    assert!(ops_in(&r, &named("t.mode")).contains(&CompareOp::Eq));
    let input = MatchInput::parse("1:p:t.point").unwrap();
    assert_eq!(input.value_type, named("t.point"));
    assert_eq!(input.to_text(), "1:p:t.point");

    let case = MatchCase::parse("1: p has_field \"label\" and m = \"auto\"").unwrap();
    let point = DataValue::Object {
        fields: [("label".to_string(), crate::ObjectField { name: "label".into(), description: String::new(), value: DataValue::String("a".into()) })].into(),
        type_name: Some("t.point".into()),
    };
    let value_of = |name: &str| match name {
        "p" => Some(point.clone()),
        "m" => Some(DataValue::String("auto".into())),
        _ => None,
    };
    assert!(case.matches(value_of));
}
