use std::time::Duration;

use ticknode_shared::text::*;
use ticknode_shared::*;

#[test]
fn defaults_hints_and_text_errors_are_stable() {
    for (ty, value, hint) in [
        (DataType::Number, DataValue::from(0i64), "—"),
        (DataType::Any, DataValue::from(0i64), "—"),
        (DataType::Bool, DataValue::Bool(true), "—"),
        (DataType::String, DataValue::from(""), "—"),
        (
            DataType::Time,
            DataValue::Time(chrono::NaiveTime::MIN),
            "HH:MM",
        ),
        (
            DataType::DateTime,
            DataValue::DateTime(chrono::DateTime::UNIX_EPOCH),
            "YYYY-MM-DD HH:MM",
        ),
        (
            DataType::Duration,
            DataValue::Duration(Duration::from_secs(1)),
            "10t / 1h 30m / 45s",
        ),
        (
            DataType::Color,
            DataValue::Color(Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }),
            "#rrggbb",
        ),
        (
            DataType::Object,
            DataValue::Object {
                fields: Default::default(),
                type_name: None,
            },
            "—",
        ),
        (
            DataType::Named("test.point".into()),
            DataValue::Object {
                fields: Default::default(),
                type_name: None,
            },
            "—",
        ),
        (
            DataType::List(Box::new(DataType::String)),
            DataValue::List {
                element_type: DataType::String,
                elements: vec![],
            },
            "—",
        ),
    ] {
        assert_eq!(default_of(&ty), value);
        assert_eq!(type_hint(&ty), hint);
    }
    for (ty, description) in [
        (DataType::Number, "a number"),
        (DataType::Bool, "true/false"),
        (DataType::Time, "a time (HH:MM or HH:MM:SS)"),
        (DataType::DateTime, "a date-time (YYYY-MM-DD HH:MM)"),
        (DataType::Duration, "a duration (10t, 90, 1h 30m, 500ms)"),
        (DataType::Color, "a color (#rrggbb)"),
    ] {
        assert_eq!(
            parse_value(" invalid ", &ty),
            Err(format!("\"invalid\" is not {description}"))
        );
    }
    for text in ["true", "так", "1"] {
        assert_eq!(
            parse_value(text, &DataType::Bool),
            Ok(DataValue::Bool(true))
        );
    }
    for text in ["false", "ні", "0"] {
        assert_eq!(
            parse_value(text, &DataType::Bool),
            Ok(DataValue::Bool(false))
        );
    }
    for ty in [
        DataType::Object,
        DataType::List(Box::new(DataType::Any)),
        DataType::Named("test.point".into()),
    ] {
        assert_eq!(
            parse_value("", &ty),
            Err(format!(
                "a value of type {} cannot be entered as text",
                data_type_name(&ty)
            ))
        );
    }
}

#[test]
fn numeric_operations_normalize_mixed_results() {
    use NumberValue::{Float, Int};
    for (a, b, difference, product) in [
        (Int(5), Int(2), Int(3), Int(10)),
        (Float(2.5), Int(2), Float(0.5), Int(5)),
        (Int(3), Float(0.5), Float(2.5), Float(1.5)),
    ] {
        assert_eq!(a - b, difference);
        assert_eq!(a * b, product);
    }
    assert_eq!(Float(2.0).normalize(), Int(2));
    assert_eq!(Int(-3).normalize(), Int(-3));
    assert_eq!(Float(f64::INFINITY).normalize(), Float(f64::INFINITY));
    assert_eq!(Float(1e30).normalize(), Float(1e30));
}

#[test]
fn invalid_colors_and_duration_parts_are_rejected() {
    for text in [
        "",
        "123456",
        "#12345",
        "#1234567",
        "#gg0000",
        "#1234界",
        "#123456zz",
    ] {
        assert_eq!(parse_color(text), None, "{text}");
    }
    for text in ["", " ", "s", "-1s", "1s 2", "1.2.3h", "5 parsecs"] {
        assert_eq!(parse_duration(text), None, "{text}");
    }
    assert_eq!(
        parse_duration("1 H 2 min 3 sec 4 MS"),
        Some(Duration::from_millis(3_723_004))
    );
    assert_eq!(
        parse_duration("1д 2год 3хв 4с 5мс"),
        Some(Duration::from_millis(93_784_005))
    );
}

#[test]
fn date_only_input_means_midnight_utc() {
    assert_eq!(
        parse_value("2026-10-29", &DataType::DateTime),
        Ok(DataValue::DateTime(
            chrono::DateTime::parse_from_rfc3339("2026-10-29T00:00:00Z")
                .unwrap()
                .to_utc()
        ))
    );
    assert_eq!(
        format_value(&parse_value("2026-10-29", &DataType::DateTime).unwrap()),
        "2026-10-29 00:00:00"
    );
}

#[test]
fn duration_formatting_handles_zero_milliseconds_and_whole_seconds() {
    for (millis, expected) in [
        (0, "0s"),
        (1, "1ms"),
        (999, "999ms"),
        (1000, "1s"),
        (1001, "1.001s"),
        (60_000, "1m"),
        (60_001, "1m 0.001s"),
    ] {
        let duration = Duration::from_millis(millis);
        assert_eq!(format_duration(duration), expected);
        assert_eq!(parse_duration(expected), Some(duration));
    }
}

#[test]
fn any_input_distinguishes_booleans_from_quoted_strings() {
    for (text, value) in [
        (" true ", DataValue::Bool(true)),
        ("false", DataValue::Bool(false)),
        ("\"true\"", DataValue::from("true")),
        ("\"false\"", DataValue::from("false")),
    ] {
        assert_eq!(parse_value(text, &DataType::Any), Ok(value));
    }
}

#[test]
fn normalization_includes_both_integer_boundaries() {
    use NumberValue::{Float, Int};
    assert_eq!(Float(i64::MIN as f64).normalize(), Int(i64::MIN));
    // i64::MAX rounds to 2^63 as f64; normalization uses Rust's saturating cast.
    assert_eq!(Float(i64::MAX as f64).normalize(), Int(i64::MAX));
    assert_eq!(
        Float((i64::MAX as f64) * 2.0).normalize(),
        Float((i64::MAX as f64) * 2.0)
    );
    assert_eq!(
        Float((i64::MIN as f64) * 2.0).normalize(),
        Float((i64::MIN as f64) * 2.0)
    );
}

#[test]
fn addition_and_division_preserve_numeric_results() {
    use NumberValue::{Float, Int};
    assert_eq!(Int(7) + Int(2), Int(9));
    assert_eq!(Int(7) + Float(0.5), Float(7.5));
    assert_eq!(Float(0.5) + Float(1.5), Int(2));
    assert_eq!(Int(9) / Int(3), Int(3));
    assert_eq!(Int(7) / Int(2), Float(3.5));
    assert_eq!(Float(1.5) / Float(0.5), Int(3));
}
