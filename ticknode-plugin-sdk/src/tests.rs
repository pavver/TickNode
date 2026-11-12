use crate::bindings::ticknode::plugin::types as wit;
use crate::convert::*;
use crate::*;

#[test]
fn input_and_struct_helpers_preserve_types_defaults_and_optional_fields() {
    let value = StructValue::new("demo.state")
        .field("number", 2i64)
        .field("text", "on")
        .field("bool", true)
        .field_opt("present", Some("yes"))
        .field_opt("absent", None::<String>)
        .build();
    assert_eq!(value.get_type(), DataType::Named("demo.state".into()));
    let inputs: Inputs = [
        (PinId(0), value.clone()),
        (PinId(1), 2.5.into()),
        (PinId(2), true.into()),
        (PinId(3), "text".into()),
    ]
    .into_iter()
    .collect();
    assert_eq!(inputs.field_number(PinId(0), "number"), Some(2.0));
    assert_eq!(inputs.field_string(PinId(0), "text"), Some("on"));
    assert_eq!(inputs.field_bool(PinId(0), "bool"), Some(true));
    assert_eq!(inputs.field(PinId(0), "present"), Some(&"yes".into()));
    assert_eq!(inputs.field(PinId(0), "absent"), None);
    for pin in [PinId(0), PinId(99)] {
        assert_eq!(inputs.number(pin, -1.0), -1.0);
        assert!(inputs.bool(pin, true));
        assert_eq!(inputs.string(pin), "");
    }
    assert_eq!(inputs.number(PinId(1), 0.0), 2.5);
    assert!(inputs.bool(PinId(2), false));
    assert_eq!(inputs.string(PinId(3)), "text");
    assert_eq!(field_number(&value, "text"), None);
    assert_eq!(field_bool(&value, "number"), None);
    assert_eq!(field_string(&value, "bool"), None);
    assert_eq!(field(&true.into(), "bool"), None);
    assert_eq!(inputs.field_bool(PinId(99), "bool"), None);
}

#[test]
fn settings_use_first_matching_key_and_reject_wrong_types_or_empty_text() {
    let settings = vec![
        ("text".into(), "hello".into()),
        ("text".into(), "later".into()),
        ("empty".into(), "".into()),
        ("number".into(), 3i64.into()),
        ("bool".into(), false.into()),
    ];
    assert_eq!(setting_str(&settings, "text"), Some("hello"));
    assert_eq!(setting_number(&settings, "number"), Some(3.0));
    assert_eq!(setting_bool(&settings, "bool"), Some(false));
    for key in ["empty", "missing", "bool", "number"] {
        assert_eq!(setting_str(&settings, key), None);
    }
    for key in ["text", "missing", "bool"] {
        assert_eq!(setting_number(&settings, key), None);
    }
    for key in ["text", "missing", "number"] {
        assert_eq!(setting_bool(&settings, key), None);
    }
}

#[test]
fn values_survive_wire_encoding_and_malformed_values_are_rejected() {
    let values = [
        true.into(),
        3i64.into(),
        2.5.into(),
        "hello".into(),
        StructValue::new("demo.state").field("n", 3i64).build(),
        DataValue::List {
            element_type: DataType::String,
            elements: vec!["x".into()],
        },
    ];
    for value in values {
        assert_eq!(decode(&encode(&value)), Some(value));
    }
    assert_eq!(decode(&[]), None);
    assert_eq!(decode(&[255]), None);
}

#[test]
fn node_definitions_keep_pin_types_directions_and_property_metadata() {
    let types = [
        DataType::Bool,
        DataType::Number,
        DataType::String,
        DataType::Time,
        DataType::DateTime,
        DataType::Duration,
        DataType::Color,
        DataType::Object,
        DataType::Any,
        DataType::Named("demo.state".into()),
    ];
    let expected = [
        wit::ScalarType::Bool,
        wit::ScalarType::Number,
        wit::ScalarType::String,
        wit::ScalarType::Time,
        wit::ScalarType::Datetime,
        wit::ScalarType::Duration,
        wit::ScalarType::Color,
        wit::ScalarType::Object,
        wit::ScalarType::Any,
        wit::ScalarType::Named("demo.state".into()),
    ];
    for (kind, expected) in types.into_iter().zip(expected) {
        let d = NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "demo/node".into(),
            title: "Title".into(),
            category: "Data".into(),
            description: "Description".into(),
            help: "Help".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(
                    1,
                    "Value",
                    DataType::List(Box::new(DataType::List(Box::new(kind)))),
                )
                .required(),
                exec_out(2, "Out"),
            ],
            properties: vec![
                PropertyDefinition::text("text", "Text"),
                PropertyDefinition::select("select", "Select", &[("v", "Label")]),
                PropertyDefinition::dynamic("dynamic", "Dynamic", "source"),
            ],
        };
        let wire = to_wit_definition(d.cost(17), true);
        assert_eq!(wire.cost, 17);
        assert_eq!(wire.type_name, "demo/node");
        assert!(wire.polls);
        assert!(matches!(wire.pins[0].pin_type, wit::PinType::Exec));
        assert!(matches!(wire.pins[2].direction, wit::PinDirection::Output));
        assert!(wire.pins[1].required);
        let wit::PinType::Data(t) = &wire.pins[1].pin_type else {
            panic!("data pin")
        };
        assert_eq!(t.list_depth, 2);
        assert_eq!(format!("{:?}", t.base), format!("{expected:?}"));
        assert!(matches!(&wire.properties[0].kind, wit::PropertyKind::Text));
        assert!(
            matches!(&wire.properties[1].kind,wit::PropertyKind::Select(v) if v == &vec![("v".into(),"Label".into())])
        );
        assert!(matches!(&wire.properties[2].kind,wit::PropertyKind::Dynamic(s) if s == "source"));
    }
}

#[test]
fn setting_conversions_keep_kind_choices_and_serialized_defaults() {
    let settings = [
        SettingDefinition::text("t", "Text"),
        SettingDefinition::number("n", "Number"),
        SettingDefinition::bool("b", "Bool"),
        SettingDefinition::secret("s", "Secret"),
        SettingDefinition::url("u", "URL"),
        SettingDefinition::select("c", "Choice", &[("x", "X")]),
    ];
    for setting in settings {
        let setting = setting
            .required()
            .described("Description")
            .default_value("x");
        let key = setting.key.clone();
        let wire = to_wit_setting(setting);
        assert_eq!(wire.key, key);
        assert!(wire.required);
        assert_eq!(wire.description, "Description");
        assert_eq!(decode(wire.default.as_ref().unwrap()), Some("x".into()));
        assert!(matches!(
            (key.as_str(), wire.kind),
            ("t", wit::SettingKind::Text)
                | ("n", wit::SettingKind::Number)
                | ("b", wit::SettingKind::Bool)
                | ("s", wit::SettingKind::Secret)
                | ("u", wit::SettingKind::Url)
                | ("c", wit::SettingKind::Select(_))
        ));
    }
}

#[test]
fn custom_types_keep_struct_fields_enum_values_and_metadata_on_the_wire() {
    let definition = TypeDefinition::new_struct(
        "demo.state",
        "State",
        vec![FieldDefinition::new("value", "Value", DataType::Number)],
    );
    let wire = to_wit_type_definition(definition);
    assert_eq!(wire.name, "demo.state");
    assert_eq!(wire.title, "State");
    let wit::TypeKind::Structure(fields) = wire.kind else {
        panic!("struct")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "value");
    assert_eq!(fields[0].title, "Value");
    assert!(matches!(fields[0].data_type.base, wit::ScalarType::Number));
    assert!(!fields[0].optional);
    let definition = TypeDefinition::new_enum("demo.mode", "Mode", &[("on", "On")]);
    let wire = to_wit_type_definition(definition);
    let wit::TypeKind::Enumeration(values) = wire.kind else {
        panic!("enum")
    };
    assert_eq!(values.len(), 1);
    assert_eq!(
        (&values[0].value, &values[0].title),
        (&"on".to_string(), &"On".to_string())
    );
}

#[test]
fn output_conversion_keeps_clears_execution_and_encoded_data_distinct() {
    assert!(matches!(
        to_wit_output(Output::Exec(PinId(7))),
        wit::Output::Exec(7)
    ));
    assert!(matches!(
        to_wit_output(Output::Clear(PinId(8))),
        wit::Output::Clear(8)
    ));
    let wit::Output::Data((pin, bytes)) = to_wit_output(Output::Data(PinId(9), "value".into()))
    else {
        panic!("data")
    };
    assert_eq!(pin, 9);
    assert_eq!(decode(&bytes), Some("value".into()));
}

#[test]
fn identifier_builder_exports_optional_dynamic_label_sources() {
    let definition =
        TypeDefinition::identifier("opaque.chat", "Chat", "An opaque chat ID").labels("chats");
    assert!(definition.is_identifier());
    let exported = crate::convert::to_wit_type_definition(definition);
    assert!(matches!(exported.kind, wit::TypeKind::Identifier(Some(source)) if source == "chats"));
    let exported = crate::convert::to_wit_type_definition(TypeDefinition::identifier(
        "opaque.user",
        "User",
        "An opaque user ID",
    ));
    assert!(matches!(exported.kind, wit::TypeKind::Identifier(None)));
}
