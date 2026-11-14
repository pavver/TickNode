use super::*;

#[test]
fn round_trips() {
    for (text, t) in [
        ("42", DataType::Number),
        ("2.5", DataType::Number),
        ("hello", DataType::String),
        ("true", DataType::Bool),
        ("07:30", DataType::Time),
        ("07:30:15", DataType::Time),
        ("2026-10-29 18:05:00", DataType::DateTime),
        ("1h 30m", DataType::Duration),
        ("500ms", DataType::Duration),
        ("1m 1.5s", DataType::Duration),
        ("1h 0.25s", DataType::Duration),
        ("#ff8800", DataType::Color),
        ("#ff880080", DataType::Color),
    ] {
        let v = parse_value(text, &t).unwrap();
        assert_eq!(v.get_type(), t);
        assert_eq!(format_value(&v), text, "{t:?}");
    }
}

#[test]
fn durations() {
    assert_eq!(
        parse_duration("90"),
        Some(chrono::TimeDelta::try_seconds(90).unwrap())
    );
    assert_eq!(
        parse_duration("-90"),
        Some(chrono::TimeDelta::try_seconds(-90).unwrap())
    );
    assert_eq!(
        parse_duration("2хв 10с"),
        Some(chrono::TimeDelta::try_seconds(130).unwrap())
    );
    assert_eq!(
        parse_duration("1.5h"),
        Some(chrono::TimeDelta::try_seconds(5400).unwrap())
    );
    assert_eq!(
        parse_duration("-1h 30m"),
        Some(chrono::TimeDelta::try_seconds(-5400).unwrap())
    );
    assert_eq!(
        parse_duration("-500ms"),
        Some(chrono::TimeDelta::try_milliseconds(-500).unwrap())
    );
    assert_eq!(
        parse_duration("−500ms"),
        Some(chrono::TimeDelta::try_milliseconds(-500).unwrap())
    );
    assert_eq!(parse_duration("5 parsecs"), None);
    assert_eq!(
        format_duration(chrono::TimeDelta::try_milliseconds(1500).unwrap()),
        "1.5s"
    );
    assert_eq!(
        format_duration(chrono::TimeDelta::try_milliseconds(-1500).unwrap()),
        "-1.5s"
    );
    assert_eq!(
        format_duration(chrono::TimeDelta::try_seconds(-5400).unwrap()),
        "-1h 30m"
    );
}

#[test]
fn errors_and_coercion() {
    assert!(parse_value("25:00", &DataType::Time).is_err());
    assert!(parse_value("red", &DataType::Color).is_err());
    assert!(parse_value("x", &DataType::Number).is_err());
    assert_eq!(
        coerce_to(DataValue::String("07:30".into()), &DataType::Time),
        DataValue::Time(parse_time("07:30").unwrap())
    );
    assert_eq!(
        coerce_to(DataValue::Number(NumberValue::Int(60)), &DataType::Duration),
        DataValue::Duration(chrono::TimeDelta::try_seconds(60).unwrap())
    );
    assert_eq!(
        coerce_to(
            DataValue::Number(NumberValue::Int(-60)),
            &DataType::Duration
        ),
        DataValue::Duration(chrono::TimeDelta::try_seconds(-60).unwrap())
    );
    assert_eq!(
        coerce_to(DataValue::String("abc".into()), &DataType::Time),
        DataValue::String("abc".into())
    );
    // Non-finite and out-of-range numbers are not converted (and do not panic)
    for n in [f64::INFINITY, f64::NEG_INFINITY, 1e30] {
        let v = DataValue::Number(NumberValue::Float(n));
        assert_eq!(coerce_to(v.clone(), &DataType::Duration), v);
    }
    assert_eq!(parse_duration("1e30"), None);
    assert_eq!(parse_duration("99999999999999999999999d"), None);
    assert_eq!(parse_any("\"5\""), DataValue::String("5".into()));
    assert_eq!(
        parse_datetime("2026-10-29T10:00:00+02:00")
            .unwrap()
            .format("%H")
            .to_string(),
        "08"
    );
}
