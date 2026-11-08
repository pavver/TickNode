use super::*;
use crate::i18n::{Locale, with_locale};

#[test]
fn inspector_values_preserve_types_and_nested_contents() {
    let values = [
        (DataValue::String("5".into()), "\"5\""),
        (DataValue::Number(NumberValue::Int(-5)), "-5"),
        (DataValue::Number(NumberValue::Float(1.25)), "1.25"),
        (DataValue::Bool(false), "false"),
        (text::parse_value("07:30", &DataType::Time).unwrap(), "07:30:00"),
        (text::parse_value("2026-01-02 07:30", &DataType::DateTime).unwrap(), "2026-01-02 07:30:00 UTC"),
        (text::parse_value("500ms", &DataType::Duration).unwrap(), "500ms"),
        (text::parse_value("#123456", &DataType::Color).unwrap(), "#123456ff"),
    ];
    for (value, expected) in values {
        assert_eq!(display_value(&value), expected);
    }
    let list = DataValue::List { element_type: DataType::Any, elements: vec![DataValue::String("x".into()), DataValue::Bool(true)] };
    assert_eq!(display_value(&list), "[\"x\", true]");
    let object = DataValue::Object {
        fields: [("items".into(), ObjectField { name: "Items".into(), description: String::new(), value: list })].into(),
        type_name: Some("test.items".into()),
    };
    assert_eq!(display_value(&object), "{items: [\"x\", true]}");
}

#[test]
fn property_labels_fall_back_without_changing_unknown_names() {
    with_locale(Locale::En, || {
        for (name, label) in [
            ("value", "Value"), ("var_name", "Variable"), ("expression", "Formula"),
            ("prefix", "Prefix"), ("label", "Label"), ("element_type", "Element type"),
            ("field_type", "Field type"), ("fields_list", "Fields (name:type, …)"),
            ("interval_ticks", "Interval, ticks"), ("all_matches", "All matches"), ("cases", "Cases"),
            ("custom_name", "custom_name"),
        ] {
            assert_eq!(property_label(name), label);
        }
    });
    with_locale(Locale::Uk, || assert_eq!(property_label("value"), "Значення"));
}

#[test]
fn relative_times_round_down_at_unit_boundaries() {
    with_locale(Locale::En, || {
        for (ms, expected) in [(-1.0, "just now"), (999.0, "just now"), (1000.0, "1 second ago"),
            (59_999.0, "59 seconds ago"), (60_000.0, "1 minute ago"),
            (3_600_000.0, "1 hour ago"), (86_400_000.0, "1 day ago")] {
            assert_eq!(relative_time(ms), expected);
        }
    });
    assert_eq!(truncate("абвг".into(), 3), "абв…");
    assert_eq!(truncate("абв".into(), 3), "абв");
}

#[test]
fn only_match_case_outputs_use_conditions_as_pin_labels() {
    let mut instance = NodeInstance {
        model: NodeModel { id: NodeId::generate(), type_name: "match".into(), position: Position::default(),
            properties: vec![("cases".into(), DataValue::List { element_type: DataType::String, elements: vec![DataValue::String("4: a > 5".into())] })] },
        definition: NodeDefinition { cost: 1, type_name: "match".into(), title: "Match".into(), category: "Control Flow".into(), description: String::new(), help: String::new(), pins: vec![], properties: vec![] },
        unavailable: None,
    };
    with_locale(Locale::En, || {
        assert_eq!(instance_pin_label(&instance, PinId(4), PinDirection::Output, "Case"), "a > 5");
        assert_eq!(instance_pin_label(&instance, PinId(0), PinDirection::Output, "Else"), "Else");
        assert_eq!(instance_pin_label(&instance, PinId(4), PinDirection::Input, "Input"), "Input");
        assert_eq!(instance_pin_label(&instance, PinId(9), PinDirection::Output, "Case"), "Case");
        instance.definition.type_name = "other".into();
        assert_eq!(instance_pin_label(&instance, PinId(4), PinDirection::Output, "Case"), "Case");
    });
}
