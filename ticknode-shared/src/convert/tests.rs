use super::*;
use crate::{Color, NumberValue};
use chrono::{DateTime, Local};
use std::time::Duration;

fn q(name: &str) -> DataType {
    DataType::Named(format!("quantity.{name}"))
}
fn run(value: DataValue, input: DataType, target: &str, unit: Option<&str>) -> Option<DataValue> {
    convert(
        &value,
        &input,
        target,
        unit,
        &TypeRegistry::from_definitions(quantity::builtin_quantities()),
    )
}
fn number_result(value: DataValue, input: DataType, target: &str, unit: Option<&str>) -> f64 {
    let DataValue::Number(n) = run(value, input, target, unit).unwrap() else {
        panic!("expected number")
    };
    n.to_f64()
}
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}
fn ids(input: DataType) -> Vec<String> {
    targets(&input, &TypeRegistry::default())
        .into_iter()
        .map(|t| t.id)
        .collect()
}

#[test]
fn targets_filter_by_declared_type_and_preserve_direction() {
    for ty in [
        DataType::Any,
        DataType::Bool,
        DataType::String,
        DataType::Time,
        DataType::Color,
        DataType::Object,
        DataType::List(Box::new(DataType::Number)),
        DataType::Named("plugin.unknown".into()),
    ] {
        assert!(ids(ty).is_empty());
    }
    assert_eq!(ids(q("temperature")), ["number"]);
    assert_eq!(ids(q("speed")), ["number", "beaufort"]);
    assert_eq!(ids(q("beaufort")), ["number", "speed"]);
    assert_eq!(ids(q("signal_strength")), ["number", "power"]);
    assert_eq!(ids(q("power")), ["number", "signal_strength"]);
    assert_eq!(ids(q("color_temperature")), ["number", "color"]);
    assert_eq!(ids(DataType::DateTime), ["time", "unix_seconds"]);
    assert_eq!(ids(DataType::Duration), ["seconds"]);
    assert!(
        !targets(&DataType::Color, &TypeRegistry::default())
            .iter()
            .any(|t| t.output == q("color_temperature"))
    );
    let number_targets = targets(&DataType::Number, &TypeRegistry::default());
    assert_eq!(number_targets.len(), quantity::catalog().len() + 2);
    for d in quantity::catalog() {
        let target = number_targets
            .iter()
            .find(|t| t.id == format!("quantity:{}", d.name))
            .unwrap();
        assert_eq!(target.output, DataType::Named(d.name.clone()));
        assert_eq!(target.unit_of.as_deref(), Some(d.name.as_str()));
        assert_eq!(target.title, d.title);
    }
    let quantity_target = &targets(&q("power"), &TypeRegistry::default())[0];
    assert_eq!(quantity_target.output, DataType::Number);
    assert_eq!(quantity_target.unit_of.as_deref(), Some("quantity.power"));
}

#[test]
fn quantity_unit_conversions_use_affine_and_inverse_curves() {
    near(
        number_result(0.0.into(), q("temperature"), "number", Some("fahrenheit")),
        32.0,
    );
    near(
        number_result(
            32.0.into(),
            DataType::Number,
            "quantity:quantity.temperature",
            Some("fahrenheit"),
        ),
        0.0,
    );
    near(
        number_result(
            250.0.into(),
            DataType::Number,
            "quantity:quantity.color_temperature",
            Some("mired"),
        ),
        4000.0,
    );
    near(
        number_result(
            4000.0.into(),
            q("color_temperature"),
            "number",
            Some("mired"),
        ),
        250.0,
    );
    // All catalog quantities accept their canonical unit without changing the value.
    for d in quantity::catalog() {
        let unit = &d.quantity().unwrap().canonical;
        near(
            number_result(
                2.0.into(),
                DataType::Number,
                &format!("quantity:{}", d.name),
                Some(unit),
            ),
            2.0,
        );
    }
    for unit in [None, Some("unknown"), Some("watt")] {
        assert_eq!(run(1.0.into(), q("temperature"), "number", unit), None);
    }
    assert_eq!(
        run(
            0.0.into(),
            DataType::Number,
            "quantity:quantity.color_temperature",
            Some("mired")
        ),
        None
    );
}

#[test]
fn wind_force_and_speed_convert_both_ways() {
    near(number_result(0.0.into(), q("speed"), "beaufort", None), 0.0);
    near(
        number_result((-1.0).into(), q("speed"), "beaufort", None),
        0.0,
    );
    near(
        number_result(6.688.into(), q("speed"), "beaufort", None),
        4.0,
    );
    near(
        number_result(1000.0.into(), q("speed"), "beaufort", None),
        12.0,
    );
    near(
        number_result(4.0.into(), q("beaufort"), "speed", None),
        6.688,
    );
    assert_eq!(run((-1.0).into(), q("beaufort"), "speed", None), None);
}

#[test]
fn logarithmic_power_conversions_reject_invalid_power() {
    near(
        number_result((-30.0).into(), q("signal_strength"), "power", None),
        1e-6,
    );
    near(
        number_result(1.0.into(), q("power"), "signal_strength", None),
        30.0,
    );
    near(
        number_result(1e-6.into(), q("power"), "signal_strength", None),
        -30.0,
    );
    for w in [0.0, -1.0] {
        assert_eq!(run(w.into(), q("power"), "signal_strength", None), None);
    }
    assert_eq!(
        run(4000.0.into(), q("signal_strength"), "power", None),
        None
    );
}

#[test]
fn kelvin_to_rgb_clamps_temperature_and_sets_alpha() {
    for (k, expected) in [
        (
            1000.0,
            Color {
                r: 255,
                g: 68,
                b: 0,
                a: 255,
            },
        ),
        (
            6500.0,
            Color {
                r: 255,
                g: 254,
                b: 250,
                a: 255,
            },
        ),
        (
            40000.0,
            Color {
                r: 152,
                g: 186,
                b: 255,
                a: 255,
            },
        ),
    ] {
        assert_eq!(
            run(k.into(), q("color_temperature"), "color", None),
            Some(DataValue::Color(expected))
        );
    }
    for (a, b) in [(0.0, 1000.0), (1e6, 40000.0)] {
        assert_eq!(
            run(a.into(), q("color_temperature"), "color", None),
            run(b.into(), q("color_temperature"), "color", None)
        );
    }
}

#[test]
fn datetime_uses_local_time_and_fractional_unix_seconds() {
    for (text, seconds) in [
        ("2024-01-02T03:04:05.250Z", 1704164645.25),
        ("1969-12-31T23:59:59.500Z", -0.5),
    ] {
        let dt = DateTime::parse_from_rfc3339(text).unwrap().to_utc();
        near(
            number_result(
                DataValue::DateTime(dt),
                DataType::DateTime,
                "unix_seconds",
                None,
            ),
            seconds,
        );
        assert_eq!(
            run(seconds.into(), DataType::Number, "datetime", None),
            Some(DataValue::DateTime(dt))
        );
        assert_eq!(
            run(DataValue::DateTime(dt), DataType::DateTime, "time", None),
            Some(DataValue::Time(dt.with_timezone(&Local).time()))
        );
    }
    assert_eq!(
        run(f64::MAX.into(), DataType::Number, "datetime", None),
        None
    );
}

#[test]
fn durations_convert_fractional_seconds_but_ticks_have_no_clock() {
    let d = DataValue::Duration(Duration::from_millis(1250));
    near(
        number_result(d.clone(), DataType::Duration, "seconds", None),
        1.25,
    );
    assert_eq!(
        run(1.25.into(), DataType::Number, "duration", None),
        Some(d)
    );
    assert_eq!(
        run(DataValue::Ticks(5), DataType::Duration, "seconds", None),
        None
    );
    for n in [-1.0, f64::MAX] {
        assert_eq!(run(n.into(), DataType::Number, "duration", None), None);
    }
}

#[test]
fn every_target_rejects_wrong_value_variants_and_nonfinite_numbers() {
    for input in [
        DataType::Number,
        q("power"),
        q("signal_strength"),
        q("speed"),
        q("beaufort"),
        q("color_temperature"),
        DataType::DateTime,
        DataType::Duration,
    ] {
        for target in targets(&input, &TypeRegistry::default()) {
            let unit = target
                .unit_of
                .as_deref()
                .and_then(quantity::kind)
                .map(|q| q.canonical.as_str());
            for value in [
                DataValue::Bool(true),
                DataValue::from("invalid"),
                DataValue::Number(NumberValue::Float(f64::NAN)),
                DataValue::Number(NumberValue::Float(f64::INFINITY)),
                DataValue::Number(NumberValue::Float(f64::NEG_INFINITY)),
            ] {
                assert_eq!(
                    run(value, input.clone(), &target.id, unit),
                    None,
                    "{}",
                    target.id
                );
            }
        }
    }
    assert_eq!(run(1.0.into(), DataType::Any, "number", None), None);
    assert_eq!(run(1.0.into(), DataType::Number, "unknown", None), None);
    assert_eq!(run(1.0.into(), q("power"), "beaufort", None), None);
}
