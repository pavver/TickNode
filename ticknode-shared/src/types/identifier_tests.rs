use crate::{
    condition::{CompareOp, MatchCase, ops_in},
    to_text::*,
    *,
};

fn registry() -> TypeRegistry {
    let mut types = TypeRegistry::new();
    types
        .insert_all(&[
            TypeDefinition::identifier("ids.chat", "Chat", "A chat").labels("chats"),
            TypeDefinition::identifier("ids.user", "User", "A user"),
            TypeDefinition::new_struct("ids.record", "Record", vec![FieldDefinition::new("chat", "Chat", chat())]),
        ])
        .unwrap();
    types
}
fn chat() -> DataType {
    DataType::Named("ids.chat".into())
}

#[test]
fn identifiers_are_opaque_integer_values_without_conversions() {
    let types = registry();
    let chat = chat();
    for other in [DataType::Number, DataType::String, DataType::Object, DataType::Named("ids.user".into())] {
        for (src, dst) in [(&chat, &other), (&other, &chat)] {
            assert!(!types.compatible(src, dst), "{src:?} -> {dst:?}");
            assert!(types.convertible(src, dst).is_none());
        }
    }
    for (src, dst) in [(&chat, &chat), (&chat, &DataType::Any), (&DataType::Any, &chat)] {
        assert!(types.compatible(src, dst));
    }
    assert!(input_value_editable(&types, &chat));
    for id in [i64::MIN, 0, i64::MAX] {
        let value = DataValue::from(id);
        assert!(input_value_fits(&types, &chat, &value));
        assert_eq!(text::parse_value_in(&types, &id.to_string(), &chat).unwrap(), value);
        assert_eq!(fit_variable_value(&types, value.clone(), &chat), Some(value));
    }
    for value in [
        DataValue::Number(NumberValue::Float(1.0)),
        "1".into(),
        true.into(),
        DataValue::Object { fields: Default::default(), type_name: None },
    ] {
        assert!(!input_value_fits(&types, &chat, &value));
        assert!(fit_variable_value(&types, value, &chat).is_none());
    }
    for text in ["1.0", "1e3", "9223372036854775808", "Alice"] {
        assert!(text::parse_value_in(&types, text, &chat).is_err());
    }
    let list = DataValue::List { element_type: DataType::Number, elements: vec![1i64.into()] };
    assert!(
        fit_typed_variable_value(
            &types,
            list,
            &DataType::List(Box::new(chat.clone())),
            &DataType::List(Box::new(DataType::Number)),
            &Default::default(),
            &quantity::CanonicalLabels
        )
        .is_none()
    );
    assert_eq!(types.default_value(&chat), 0i64.into());
    let bad = TypeDefinition::identifier("ids.bad", "Bad", "").labels("");
    assert!(types.clone().insert_all(&[bad]).is_err());
}

#[test]
fn identifier_presentation_preserves_ids_and_resolves_nested_labels() {
    let types = registry();
    let id = 9_007_199_254_740_993i64;
    let labels = |name: &str, found: i64| (name == "ids.chat" && found == id).then(|| "Alice".into());
    let rules =
        DisplayRules { number: NumberDisplay { rounding: Rounding::Decimals, digits: 2 }, ..Default::default() };
    let show = |v: &DataValue, t: &DataType| {
        to_text_with_identifiers(v, t, &Default::default(), &rules, &quantity::CanonicalLabels, &types, &labels)
    };
    assert_eq!(show(&id.into(), &chat()), "Alice");
    assert_eq!(show(&i64::MIN.into(), &chat()), i64::MIN.to_string());
    assert_eq!(
        to_text_in(&id.into(), &chat(), &Default::default(), &rules, &quantity::CanonicalLabels, &types),
        id.to_string()
    );
    let list = DataValue::List { element_type: chat(), elements: vec![id.into(), 4i64.into()] };
    assert_eq!(show(&list, &DataType::Any), "Alice, 4");
    let record = DataValue::Object {
        type_name: Some("ids.record".into()),
        fields: [("chat".into(), ObjectField { name: "Chat".into(), description: String::new(), value: id.into() })]
            .into(),
    };
    assert_eq!(show(&record, &DataType::Any), "{chat: Alice}");
}

#[test]
fn match_identifiers_use_exact_equality_and_reject_foreign_types_and_ordering() {
    let types = registry();
    let id = 9_007_199_254_740_993i64;
    let matches = |source: &str, right: DataType| {
        MatchCase::parse(source).unwrap().matches_in(
            &types,
            |name| if name == "a" { chat() } else { right.clone() },
            |_| Some(id.into()),
        )
    };
    assert!(matches("1: a = 9007199254740993", chat()));
    assert!(!matches("1: a = 9007199254740992", chat()));
    assert!(matches("1: a != 9007199254740992", chat()));
    assert!(!matches("1: a > 0", chat()));
    assert!(!matches("1: a = 1.0", chat()));
    assert!(matches("1: a = b", chat()));
    for literal in ["9223372036854775808", "-9223372036854775809"] {
        let case = MatchCase::parse(&format!("1: a = {literal}")).unwrap();
        for id in [i64::MIN, i64::MAX] {
            assert!(!case.matches_in(&types, |_| chat(), |_| Some(id.into())));
        }
    }
    for t in [DataType::Number, DataType::Named("ids.user".into())] {
        assert!(!matches("1: a = b", t.clone()));
        assert!(!matches("1: a != b", t));
    }
    assert_eq!(ops_in(&types, &chat()), &[CompareOp::Eq, CompareOp::Ne, CompareOp::HasValue, CompareOp::NoValue]);
}

#[test]
fn identifier_kind_and_ownership_roundtrip_without_reindexing_old_kinds() {
    let mut def = TypeDefinition::identifier("ids.chat", "Chat", "A chat").labels("chats");
    def.plugin = Some("messaging".into());
    let encoded = postcard::to_allocvec(&def).unwrap();
    assert_eq!(postcard::from_bytes::<TypeDefinition>(&encoded).unwrap(), def);
    for (kind, index) in
        [(TypeKind::Struct { fields: vec![] }, 0), (TypeKind::Enum { values: vec![] }, 1), (def.kind, 3)]
    {
        assert_eq!(postcard::to_allocvec(&kind).unwrap()[0], index);
    }
}
