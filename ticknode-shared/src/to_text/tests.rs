use super::*;
use quantity::CanonicalLabels;

#[test]
fn every_runtime_type_has_a_readable_presentation() {
    let rules = DisplayRules::default();
    let options = ToTextOptions::default();
    let show = |v: DataValue| to_text(&v, &DataType::Any, &options, &rules, &CanonicalLabels);
    assert_eq!(show(true.into()), "true");
    assert_eq!(show(1.975.into()), "1.98");
    assert_eq!(show("hello".into()), "hello");
    assert_eq!(
        show(DataValue::Time(
            chrono::NaiveTime::from_hms_opt(7, 30, 12).unwrap()
        )),
        "07:30"
    );
    assert_eq!(
        show(DataValue::Duration(std::time::Duration::from_secs(5400))),
        "1h 30m"
    );
    assert_eq!(show(DataValue::Ticks(12)), "12t");
    assert_eq!(
        show(DataValue::Color(Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        })),
        "#0c2238"
    );
    let object = DataValue::Object {
        fields: [(
            "x".into(),
            ObjectField {
                name: "x".into(),
                description: String::new(),
                value: 2i64.into(),
            },
        )]
        .into(),
        type_name: None,
    };
    assert_eq!(show(object), "{x: 2}");
    let options = ToTextOptions {
        datetime_format: "%Y".into(),
        ..Default::default()
    };
    let dt = DataValue::DateTime(
        chrono::DateTime::parse_from_rfc3339("2024-06-15T12:00:00Z")
            .unwrap()
            .to_utc(),
    );
    assert_eq!(
        to_text(&dt, &DataType::DateTime, &options, &rules, &CanonicalLabels),
        "2024"
    );
}

#[test]
fn options_and_declared_list_elements_are_respected() {
    let power = DataType::Named("quantity.power".into());
    let rules = DisplayRules::default();
    let mut o = ToTextOptions::default();
    let show =
        |v: &DataValue, t: &DataType, o: &ToTextOptions| to_text(v, t, o, &rules, &CanonicalLabels);
    assert_eq!(show(&1970i64.into(), &power, &o), "1.97 kW");
    o.show_units = false;
    assert_eq!(show(&1970i64.into(), &power, &o), "1.97");
    o.unit = Some("watt".into());
    assert_eq!(show(&1970i64.into(), &power, &o), "1970");
    o.unit = None;
    o.digits = Some(2);
    assert_eq!(show(&1970i64.into(), &power, &o), "2");
    assert_eq!(
        show(
            &DataValue::Duration(std::time::Duration::from_secs(5400)),
            &DataType::Duration,
            &o
        ),
        "5400"
    );
    o.true_text = "on".into();
    o.false_text = "off".into();
    assert_eq!(show(&true.into(), &DataType::Bool, &o), "on");
    assert_eq!(show(&false.into(), &DataType::Bool, &o), "off");
    o = Default::default();
    let list = DataValue::List {
        element_type: DataType::Number,
        elements: vec![1970i64.into(), 57i64.into()],
    };
    assert_eq!(
        show(&list, &DataType::List(Box::new(power)), &o),
        "1.97 kW, 57 W"
    );
    assert_eq!(
        show(&"member".into(), &DataType::Named("test.enum".into()), &o),
        "member"
    );
}

#[test]
fn invalid_or_inapplicable_time_formats_fall_back_without_panicking() {
    let time = DataValue::Time(chrono::NaiveTime::from_hms_opt(7, 30, 0).unwrap());
    for format in ["%Q", "%Y"] {
        let options = ToTextOptions {
            datetime_format: format.into(),
            ..Default::default()
        };
        assert_eq!(
            to_text(
                &time,
                &DataType::Time,
                &options,
                &Default::default(),
                &CanonicalLabels
            ),
            "07:30"
        );
    }
}

#[test]
fn struct_fields_use_their_declared_quantity_types() {
    let types = TypeRegistry::from_definitions([TypeDefinition::new_struct(
        "test.reading",
        "Reading",
        vec![FieldDefinition::new(
            "power",
            "Power",
            DataType::Named("quantity.power".into()),
        )],
    )]);
    let value = DataValue::Object {
        type_name: Some("test.reading".into()),
        fields: [
            (
                "power".into(),
                ObjectField {
                    name: "Power".into(),
                    description: String::new(),
                    value: 1970i64.into(),
                },
            ),
            (
                "label".into(),
                ObjectField {
                    name: "Label".into(),
                    description: String::new(),
                    value: "Kitchen".into(),
                },
            ),
        ]
        .into(),
    };
    assert_eq!(
        to_text_in(
            &value,
            &DataType::Any,
            &Default::default(),
            &Default::default(),
            &CanonicalLabels,
            &types
        ),
        "{label: \"Kitchen\", power: 1.97 kW}"
    );
}

#[test]
fn integer_text_keeps_all_digits() {
    assert_eq!(
        to_text(
            &i64::MAX.into(),
            &DataType::Number,
            &Default::default(),
            &Default::default(),
            &CanonicalLabels
        ),
        i64::MAX.to_string()
    );
}
