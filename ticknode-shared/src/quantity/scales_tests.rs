use super::*;
use crate::{DataType, DataValue, TypeRegistry, condition::MatchCase};

fn named(suffix: &str) -> DataType {
    DataType::Named(format!("quantity.{suffix}"))
}

#[test]
fn currency_catalog_has_independent_codes_units_and_rounding() {
    let codes = "UAH USD EUR GBP PLN CHF CZK HUF RON BGN SEK NOK DKK ISK TRY GEL MDL KZT ILS JPY CNY KRW INR CAD AUD NZD SGD HKD BRL MXN ZAR AED";
    let currencies: Vec<_> = catalog()
        .iter()
        .filter(|d| d.name.starts_with("quantity.currency_"))
        .collect();
    assert_eq!(currencies.len(), codes.split_whitespace().count());
    for code in codes.split_whitespace() {
        let suffix = code.to_ascii_lowercase();
        let definition = currencies
            .iter()
            .find(|d| d.name == format!("quantity.currency_{suffix}"))
            .unwrap();
        let q = definition.quantity().unwrap();
        assert_eq!(
            definition.description,
            format!("Money in {code}; currencies never convert")
        );
        assert_eq!(definition.color.as_deref(), Some("#5aa469"));
        assert_eq!(q.units.len(), 1);
        assert_eq!(q.canonical, suffix);
        assert_eq!(q.units[0].symbol, code);
        assert_eq!(q.auto, [suffix.clone()]);
        assert_eq!(
            q.decimals,
            Some(if matches!(code, "JPY" | "KRW" | "HUF") {
                0
            } else {
                2
            })
        );
        for symbol in [code, &suffix] {
            assert_eq!(unit_by_symbol(symbol).unwrap().0, definition.name);
        }
    }
    for (symbol, code) in [
        ("грн", "uah"),
        ("₴", "uah"),
        ("€", "eur"),
        ("євро", "eur"),
        ("£", "gbp"),
        ("zł", "pln"),
    ] {
        assert_eq!(
            unit_by_symbol(symbol).unwrap().0,
            format!("quantity.currency_{code}")
        );
    }
    for ambiguous in ["$", "¥", "kr"] {
        assert!(unit_by_symbol(ambiguous).is_none());
    }
}

#[test]
fn currencies_cannot_connect_or_convert_into_one_another() {
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    let currencies: Vec<_> = catalog()
        .iter()
        .filter(|d| d.name.starts_with("quantity.currency_"))
        .map(|d| DataType::Named(d.name.clone()))
        .collect();
    for a in &currencies {
        assert!(registry.compatible(a, &DataType::Number));
        assert!(registry.convertible(&DataType::Number, a).is_some());
        for b in &currencies {
            assert_eq!(registry.compatible(a, b), a == b);
            assert!(registry.convertible(a, b).is_none());
            assert_eq!(
                registry.compatible(
                    &DataType::List(Box::new(a.clone())),
                    &DataType::List(Box::new(b.clone()))
                ),
                a == b
            );
            assert!(
                registry
                    .convertible(
                        &DataType::List(Box::new(a.clone())),
                        &DataType::List(Box::new(b.clone()))
                    )
                    .is_none()
            );
        }
    }
}

#[test]
fn match_rejects_foreign_currencies_in_inputs_and_literals_including_inequality() {
    let registry = TypeRegistry::from_definitions(builtin_quantities());
    for op in ["=", "!=", "<", "<=", ">", ">="] {
        for right in ["b", "10 USD"] {
            let case = MatchCase::parse(&format!("1: a {op} {right}")).unwrap();
            assert!(
                !case.matches_in(
                    &registry,
                    |name| named(if name == "a" {
                        "currency_uah"
                    } else {
                        "currency_usd"
                    }),
                    |_| Some(10.into())
                ),
                "{op} {right}"
            );
        }
    }
    let evaluate = |text: &str| {
        MatchCase::parse(text).unwrap().matches_in(
            &registry,
            |_| named("currency_uah"),
            |_| Some(10.into()),
        )
    };
    assert!(evaluate("1: a = 10 UAH"));
    assert!(evaluate("1: a = b"));
    assert!(evaluate("1: a = 10 USD or a = 10 UAH"));
    assert!(!evaluate("1: a = 10 USD and a = 10 UAH"));
}

#[test]
fn quantity_defaults_use_fixed_decimals_only_when_specified() {
    for (name, expected) in [
        ("power", None),
        ("currency_uah", Some(2)),
        ("currency_jpy", Some(0)),
        ("beaufort", Some(0)),
        ("aqi", Some(0)),
        ("ph", Some(1)),
        ("energy_consumption", None),
        ("color_temperature", Some(0)),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        assert_eq!(q.decimals, expected);
        let rule = QuantityDisplay::default_for(q);
        assert_eq!(rule.digits, expected.unwrap_or(3));
        assert_eq!(
            rule.rounding,
            if expected.is_some() {
                Rounding::Decimals
            } else {
                Rounding::Significant
            }
        );
        assert_eq!(
            DisplayRules::default().effective(&format!("quantity.{name}"), q),
            rule
        );
    }
    let legacy: QuantityKind =
        serde_json::from_str(r#"{"canonical":"unit","units":[],"auto":[]}"#).unwrap();
    assert_eq!(legacy.decimals, None);
    let q = kind("quantity.currency_uah").unwrap();
    let bytes = postcard::to_allocvec(q).unwrap();
    assert_eq!(&postcard::from_bytes::<QuantityKind>(&bytes).unwrap(), q);
}

#[test]
fn remaining_scales_and_appended_data_units_have_canonical_factors() {
    for (name, symbol, factor) in [
        ("beaufort", "Bft", 1.0),
        ("aqi", "AQI", 1.0),
        ("ph", "pH", 1.0),
        ("energy_consumption", "kWh/100km", 1.0),
        ("energy_consumption", "Wh/km", 0.1),
        ("data_size", "EB", 1e18),
        ("data_size", "ZB", 1e21),
        ("data_size", "YB", 1e24),
        ("data_size", "EiB", 2f64.powi(60)),
        ("data_size", "ZiB", 2f64.powi(70)),
        ("data_size", "YiB", 2f64.powi(80)),
    ] {
        let q = kind(&format!("quantity.{name}")).unwrap();
        let unit = unit_in(q, symbol, false).unwrap();
        assert_eq!(to_canonical(1.0, unit), factor);
        assert_eq!(
            parse_quantity(&format!("2 {symbol}"), q),
            Some(2.0 * factor)
        );
        assert_eq!(from_canonical(2.0 * factor, unit), 2.0);
    }
    let q = kind("quantity.data_size").unwrap();
    let ids: Vec<_> = q.units.iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, "byte bit kilobit megabit gigabit kilobyte megabyte gigabyte terabyte petabyte kibibyte mebibyte gibibyte tebibyte pebibyte exabyte zettabyte yottabyte exbibyte zebibyte yobibyte".split_whitespace().collect::<Vec<_>>());
    assert_eq!(crate::text::format_value(&DataValue::from(10)), "10");
}
