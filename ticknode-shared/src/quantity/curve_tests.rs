use super::*;
use crate::{DataType, TypeRegistry, condition::MatchCase};

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs().max(1.0) * 1e-12,
        "{actual} != {expected}"
    );
}

#[test]
fn nonlinear_units_round_trip_through_canonical_numbers() {
    for (name, symbol, written, canonical) in [
        ("color_temperature", "mired", 250.0, 4000.0),
        ("color_temperature", "майред", 400.0, 2500.0),
        ("energy_consumption", "km/kWh", 5.0, 20.0),
        ("energy_consumption", "mi/kWh", 5.0, 100.0 / 1.609344 / 5.0),
        ("signal_strength", "mW", 1.0, 0.0),
        ("signal_strength", "mW", 0.01, -20.0),
        ("signal_strength", "W", 1.0, 30.0),
        ("signal_strength", "W", 0.001, 0.0),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        let unit = unit_in(q, symbol, false).unwrap();
        close(to_canonical(written, unit), canonical);
        close(from_canonical(canonical, unit), written);
        close(
            parse_quantity(&format!("{written} {symbol}"), q).unwrap(),
            canonical,
        );
    }
    assert!(kind("quantity.energy_efficiency").is_none());
}

#[test]
fn parsing_rejects_undefined_and_overflowing_conversions_including_literals() {
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    for (name, literals) in [
        (
            "color_temperature",
            vec!["0 mired", "-0 mired", "1e-310 mired"],
        ),
        ("energy_consumption", vec!["0 km/kWh", "0 mi/kWh"]),
        (
            "signal_strength",
            vec!["0 mW", "-1 mW", "0 W", "-1 W", "1e308 W"],
        ),
    ] {
        let name = format!("quantity.{name}");
        let q = kind(&name).unwrap();
        for literal in literals {
            assert!(parse_quantity(literal, q).is_none(), "{literal}");
            assert!(parse_literal(literal, q).is_none(), "{literal}");
            assert!(
                crate::text::parse_value_in(&registry, literal, &DataType::Named(name.clone()))
                    .is_err()
            );
        }
    }
}

#[test]
fn match_compares_color_temperature_literals_in_kelvin() {
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    for (expression, expected) in [
        ("t = 250 mired", true),
        ("t > 250 mired", false),
        ("t < 200 mired", true),
        ("t != 0 mired", false),
    ] {
        let case = MatchCase::parse(&format!("1: {expression}")).unwrap();
        assert_eq!(
            case.matches_in(
                &registry,
                |_| DataType::Named("quantity.color_temperature".into()),
                |_| Some(4000.into())
            ),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn display_selects_affine_defaults_and_allows_explicit_nonlinear_units() {
    for (name, canonical, explicit, expected) in [
        ("color_temperature", 4000.0, "mired", "250 mired"),
        (
            "energy_consumption",
            20.0,
            "kilometer_per_kilowatt_hour",
            "5 km/kWh",
        ),
        ("signal_strength", 30.0, "signal_watt", "1 W"),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        let rule = QuantityDisplay::default_for(q);
        assert!(
            rule.steps.iter().all(
                |s| q.units.iter().find(|u| u.id == s.unit).unwrap().curve == UnitCurve::Affine
            )
        );
        let format = QuantityFormat {
            unit: Some(explicit.into()),
            ..Default::default()
        };
        assert_eq!(
            format_quantity(canonical, q, &format, &rule, &CanonicalLabels),
            expected
        );
        if name == "color_temperature" {
            assert_eq!(
                format_quantity(canonical, q, &Default::default(), &rule, &CanonicalLabels),
                "4000 K"
            );
            assert!(format_quantity(0.0, q, &format, &rule, &CanonicalLabels).is_empty());
        }
        if name == "signal_strength" {
            assert!(format_quantity(4000.0, q, &format, &rule, &CanonicalLabels).is_empty());
        }
        let mut q = q.clone();
        q.auto.extend(
            q.units
                .iter()
                .filter(|u| u.curve != UnitCurve::Affine)
                .map(|u| u.id.clone()),
        );
        assert_eq!(QuantityDisplay::default_for(&q), rule);
    }
}

#[test]
fn ambiguous_symbols_require_a_quantity_context() {
    for symbol in ["K", "kelvin", "К"] {
        assert!(unit_by_symbol(symbol).is_none(), "{symbol}");
    }
    let temperature = kind("quantity.temperature").unwrap();
    close(parse_quantity("300 K", temperature).unwrap(), 26.85);
    assert_eq!(
        parse_quantity("300 K", kind("quantity.color_temperature").unwrap()),
        Some(300.0)
    );
    assert_eq!(unit_by_symbol("MW").unwrap().0, "quantity.power");
}

#[test]
fn curves_default_in_legacy_json_and_round_trip_in_postcard() {
    let legacy = r#"{"id":"unit","symbol":"u","factor":1.0,"offset":0.0,"aliases":[]}"#;
    let mut unit: UnitDefinition = serde_json::from_str(legacy).unwrap();
    assert_eq!(unit.curve, UnitCurve::Affine);
    for curve in [
        UnitCurve::Affine,
        UnitCurve::Inverse,
        UnitCurve::Log10 { scale: 10.0 },
    ] {
        unit.curve = curve;
        assert_eq!(
            serde_json::from_str::<UnitDefinition>(&serde_json::to_string(&unit).unwrap()).unwrap(),
            unit
        );
        assert_eq!(
            postcard::from_bytes::<UnitDefinition>(&postcard::to_allocvec(&unit).unwrap()).unwrap(),
            unit
        );
    }
    unit.factor = 2.0;
    unit.offset = 7.0;
    unit.curve = UnitCurve::Log10 { scale: 20.0 };
    close(to_canonical(5.0, &unit), 27.0);
    close(from_canonical(27.0, &unit), 5.0);
    unit.curve = UnitCurve::Inverse;
    close(to_canonical(5.0, &unit), 0.4);
    close(from_canonical(0.4, &unit), 5.0);
}

#[test]
fn match_resolves_ambiguous_suffixes_using_the_declared_input_type() {
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    for (name, expression, canonical) in [
        ("temperature", "t < 300 K", 20.0),
        ("color_temperature", "t = 4000 K", 4000.0),
        ("power", "t = 2 W", 2.0),
        ("signal_strength", "t = 1 mW", 0.0),
        ("signal_strength", "t = 1 W", 30.0),
    ] {
        let case = MatchCase::parse(&format!("1: {expression}")).unwrap();
        assert!(
            case.matches_in(
                &registry,
                |_| DataType::Named(format!("quantity.{name}")),
                |_| Some(canonical.into())
            ),
            "{name}/{expression}"
        );
    }
}

#[test]
fn linear_units_win_over_non_linear_aliases() {
    assert_eq!(
        crate::quantity::unit_by_symbol("W").map(|(n, _)| n),
        Some("quantity.power")
    );
    assert_eq!(
        crate::quantity::unit_by_symbol("mW").map(|(n, _)| n),
        Some("quantity.power")
    );
}
