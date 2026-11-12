use super::*;

#[test]
fn extended_units_have_expected_canonical_factors() {
    for (name, id, expected) in [
        ("power", "gigawatt", 1e9),
        ("power", "terawatt", 1e12),
        ("power", "btu_per_hour", 0.29307107),
        ("energy", "gigawatt_hour", 1e9),
        ("energy", "terawatt_hour", 1e12),
        ("energy", "milliwatt_hour", 0.001),
        ("energy", "gigajoule", 1e9 / 3600.0),
        ("energy", "calorie", 4.184 / 3600.0),
        ("energy", "kilocalorie", 4184.0 / 3600.0),
        ("energy", "megacalorie", 4.184e6 / 3600.0),
        ("energy", "gigacalorie", 4.184e9 / 3600.0),
        ("voltage", "microvolt", 1e-6),
        ("voltage", "megavolt", 1e6),
        ("apparent_power", "millivolt_ampere", 0.001),
        ("pressure", "centibar", 1000.0),
        ("pressure", "inch_of_water", 249.08891),
        ("speed", "inch_per_day", 0.0254 / 86400.0),
        ("speed", "inch_per_hour", 0.0254 / 3600.0),
        ("speed", "inch_per_second", 0.0254),
        ("speed", "millimeter_per_day", 0.001 / 86400.0),
        ("speed", "millimeter_per_second", 0.001),
        ("length", "nautical_mile", 1852.0),
        ("volume", "fluid_ounce", 0.0295735295625),
        ("volume", "ccf", 2831.6846592),
        ("volume", "mcf", 28316.846592),
        ("mass", "microgram", 1e-9),
        ("mass", "stone", 6.35029318),
        ("irradiance", "btu_per_hour_square_foot", 3.15459075),
        ("concentration", "gram_per_cubic_meter", 1e6),
        ("reactive_power", "var", 1.0),
        ("reactive_power", "kilovar", 1000.0),
        ("reactive_power", "millivar", 0.001),
        ("reactive_energy", "varh", 1.0),
        ("reactive_energy", "kvarh", 1000.0),
        ("area", "square_meter", 1.0),
        ("area", "square_centimeter", 1e-4),
        ("area", "square_millimeter", 1e-6),
        ("area", "square_kilometer", 1e6),
        ("area", "square_inch", 0.00064516),
        ("area", "square_foot", 0.09290304),
        ("area", "square_yard", 0.83612736),
        ("area", "square_mile", 2589988.110336),
        ("area", "acre", 4046.8564224),
        ("area", "hectare", 10000.0),
        ("data_rate", "bit_per_second", 1.0),
        ("data_rate", "kilobit_per_second", 1e3),
        ("data_rate", "megabit_per_second", 1e6),
        ("data_rate", "gigabit_per_second", 1e9),
        ("data_rate", "byte_per_second", 8.0),
        ("data_rate", "kilobyte_per_second", 8e3),
        ("data_rate", "megabyte_per_second", 8e6),
        ("data_rate", "gigabyte_per_second", 8e9),
        ("data_rate", "kibibyte_per_second", 8192.0),
        ("data_rate", "mebibyte_per_second", 8388608.0),
        ("data_rate", "gibibyte_per_second", 8589934592.0),
        ("data_size", "byte", 1.0),
        ("data_size", "bit", 0.125),
        ("data_size", "kilobit", 125.0),
        ("data_size", "megabit", 125000.0),
        ("data_size", "gigabit", 125000000.0),
        ("data_size", "kilobyte", 1e3),
        ("data_size", "megabyte", 1e6),
        ("data_size", "gigabyte", 1e9),
        ("data_size", "terabyte", 1e12),
        ("data_size", "petabyte", 1e15),
        ("data_size", "kibibyte", 1024.0),
        ("data_size", "mebibyte", 1048576.0),
        ("data_size", "gibibyte", 1073741824.0),
        ("data_size", "tebibyte", 1099511627776.0),
        ("data_size", "pebibyte", 1125899906842624.0),
        ("precipitation_intensity", "millimeter_per_hour", 1.0),
        ("precipitation_intensity", "millimeter_per_day", 1.0 / 24.0),
        ("precipitation_intensity", "inch_per_hour", 25.4),
        ("precipitation_intensity", "inch_per_day", 25.4 / 24.0),
        ("volume_flow_rate", "cubic_meter_per_hour", 1.0),
        ("volume_flow_rate", "cubic_meter_per_second", 3600.0),
        ("volume_flow_rate", "cubic_foot_per_minute", 1.69901079552),
        ("volume_flow_rate", "liter_per_hour", 0.001),
        ("volume_flow_rate", "liter_per_minute", 0.06),
        ("volume_flow_rate", "liter_per_second", 3.6),
        ("volume_flow_rate", "gallon_per_minute", 0.22712470704),
        ("volume_flow_rate", "milliliter_per_second", 0.0036),
        ("conductivity", "microsiemens_per_centimeter", 1.0),
        ("conductivity", "millisiemens_per_centimeter", 1000.0),
        ("conductivity", "siemens_per_centimeter", 1e6),
        ("signal_strength", "decibel_milliwatt", 1.0),
        ("sound_pressure", "decibel", 1.0),
        ("sound_pressure", "a_weighted_decibel", 1.0),
        ("angle", "degree", 1.0),
        ("angle", "radian", 180.0 / std::f64::consts::PI),
        ("blood_glucose", "milligram_per_deciliter", 1.0),
        ("blood_glucose", "millimole_per_liter", 18.0),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        let unit = q.units.iter().find(|unit| unit.id == id).unwrap();
        assert!(
            (to_canonical(1.0, unit) - expected).abs() <= expected.abs() * 1e-12,
            "{name}/{id}"
        );
        for value in [0.0, -2.5, 1.0, 1234.5] {
            let round_trip = from_canonical(to_canonical(value, unit), unit);
            assert!((round_trip - value).abs() < 1e-9, "{name}/{id}");
        }
    }
}

#[test]
fn catalog_symbols_and_aliases_resolve_locally_and_globally_when_unambiguous() {
    for definition in catalog() {
        let q = definition.quantity().unwrap();
        for unit in &q.units {
            for symbol in std::iter::once(&unit.symbol).chain(&unit.aliases) {
                assert_eq!(
                    unit_in(q, symbol, false).unwrap().id,
                    unit.id,
                    "{}/{symbol}",
                    definition.name
                );
                let occurrences = catalog()
                    .iter()
                    .flat_map(|d| &d.quantity().unwrap().units)
                    .filter(|u| {
                        std::iter::once(&u.symbol)
                            .chain(&u.aliases)
                            .any(|s| s == symbol)
                    })
                    .map(|u| u.curve == crate::quantity::UnitCurve::Affine)
                    .collect::<Vec<_>>();
                // Ambiguous symbols resolve only to the single linear unit, if there is one.
                let linear = occurrences.iter().filter(|a| **a).count();
                let expected = (occurrences.len() == 1
                    || (linear == 1 && unit.curve == crate::quantity::UnitCurve::Affine))
                    .then_some((definition.name.as_str(), unit.id.as_str()));
                if expected.is_some() || linear != 1 {
                    assert_eq!(
                        unit_by_symbol(symbol).map(|(n, u)| (n, u.id.as_str())),
                        expected,
                        "{symbol}"
                    );
                }
            }
        }
    }
    assert_eq!(unit_by_symbol("dB").unwrap().0, "quantity.sound_pressure");
    assert_eq!(unit_by_symbol("dBm").unwrap().0, "quantity.signal_strength");
}

#[test]
fn canonical_units_and_existing_unit_order_are_preserved() {
    for (name, ids) in [
        ("power", "watt kilowatt megawatt milliwatt"),
        (
            "energy",
            "watt_hour kilowatt_hour megawatt_hour joule kilojoule megajoule",
        ),
        ("voltage", "volt millivolt kilovolt"),
        ("apparent_power", "volt_ampere kilovolt_ampere"),
        (
            "pressure",
            "pascal hectopascal kilopascal millibar bar psi mmhg inhg",
        ),
        (
            "speed",
            "meter_per_second kilometer_per_hour mile_per_hour knot foot_per_second",
        ),
        (
            "length",
            "meter millimeter centimeter kilometer inch foot yard mile",
        ),
        ("volume", "liter milliliter cubic_meter gallon cubic_foot"),
        ("mass", "kilogram gram milligram tonne pound ounce"),
        ("irradiance", "watt_per_square_meter"),
        (
            "concentration",
            "microgram_per_cubic_meter milligram_per_cubic_meter",
        ),
        ("percentage", "percent"),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        let expected: Vec<_> = ids.split_whitespace().collect();
        assert_eq!(q.canonical, expected[0]);
        let actual: Vec<_> = q
            .units
            .iter()
            .take(expected.len())
            .map(|u| u.id.as_str())
            .collect();
        assert_eq!(actual, expected, "{name}");
    }
}

#[test]
fn byte_unit_literals_are_valid_match_operands() {
    use crate::{DataType, DataValue, TypeRegistry, condition::MatchCase};
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    let case = MatchCase::parse("1: size > 1 B").unwrap();
    assert!(case.matches_in(
        &registry,
        |_| DataType::Named("quantity.data_size".into()),
        |_| Some(DataValue::from(2))
    ));
}
