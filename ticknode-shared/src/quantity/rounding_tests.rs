use super::*;
use crate::{
    DataType, DataValue, NumberValue, ServerSettings,
    to_text::{ToTextOptions, to_text},
};

struct Comma;
impl QuantityLabels for Comma {
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
    fn decimal_separator(&self) -> String {
        ",".into()
    }
}

#[test]
fn decimal_format_preserves_zeros_and_normalizes_negative_zero() {
    for (value, digits, expected) in [
        (23.5, 2, "23.50"),
        (-0.001, 2, "0.00"),
        (-0.0, 2, "0.00"),
        (-0.1, 0, "0"),
        (23.5, 0, "24"),
        (0.0, 3, "0.000"),
    ] {
        assert_eq!(
            format_rounded(value, Rounding::Decimals, digits, &CanonicalLabels),
            expected
        );
    }
    assert_eq!(format_rounded(23.5, Rounding::Decimals, 2, &Comma), "23,50");
    assert_eq!(
        format_rounded(23.5, Rounding::Significant, 4, &Comma),
        "23,5"
    );
}

#[test]
fn decimal_units_use_raw_magnitude_and_node_mode_overrides_project_selection() {
    let q = kind("quantity.power").unwrap();
    let mut rule = QuantityDisplay::default_for(q);
    rule.rounding = Rounding::Decimals;
    rule.digits = 2;
    for value in [999.7, -999.7] {
        assert_eq!(rule.unit(value, q, 2).id, "watt");
    }
    assert_eq!(rule.unit(1000.0, q, 2).id, "kilowatt");
    let rules = DisplayRules {
        quantities: [("quantity.power".into(), rule)].into(),
        ..Default::default()
    };
    let ty = DataType::Named("quantity.power".into());
    let show = |n: f64, options: &ToTextOptions| {
        to_text(&DataValue::from(n), &ty, options, &rules, &CanonicalLabels)
    };
    let mut options = ToTextOptions::default();
    assert_eq!(show(999.7, &options), "999.70 W");
    options.rounding = Some(Rounding::Significant);
    assert_eq!(show(999.7, &options), "1 kW");
    options.rounding = None;
    options.digits = Some(3);
    assert_eq!(show(999.7, &options), "1 kW");
    options.rounding = Some(Rounding::Decimals);
    options.digits = None;
    options.show_units = false;
    assert_eq!(show(999.7, &options), "999.70");
    options.unit = Some("kilowatt".into());
    assert_eq!(show(999.7, &options), "1.00");

    let project = DisplayRules::default();
    assert_eq!(
        to_text(
            &999.7.into(),
            &ty,
            &ToTextOptions {
                rounding: Some(Rounding::Decimals),
                ..Default::default()
            },
            &project,
            &CanonicalLabels
        ),
        "999.70 W"
    );
}

#[test]
fn plain_numbers_resolve_project_overrides_and_legacy_digits() {
    let rules = DisplayRules {
        number: NumberDisplay {
            rounding: Rounding::Decimals,
            digits: 4,
        },
        ..Default::default()
    };
    let show = |v: DataValue, o: &ToTextOptions| to_text(&v, &DataType::Number, o, &rules, &Comma);
    let mut options = ToTextOptions::default();
    assert_eq!(
        show(DataValue::Number(NumberValue::Float(23.5)), &options),
        "23,5000"
    );
    assert_eq!(show(23i64.into(), &options), "23,0000");
    assert_eq!(
        show(i64::MAX.into(), &options),
        format!("{},0000", i64::MAX)
    );
    options.rounding = Some(Rounding::Decimals);
    assert_eq!(show(23i64.into(), &options), "23,00");
    options.digits = Some(0);
    assert_eq!(show(23i64.into(), &options), "23");
    options.rounding = Some(Rounding::Significant);
    options.digits = None;
    assert_eq!(show(1.23456.into(), &options), "1,23");
    assert_eq!(show(i64::MAX.into(), &options), i64::MAX.to_string());
    options.rounding = None;
    options.digits = Some(2);
    assert_eq!(show(1.23456.into(), &options), "1,2");
    options.digits = Some(0);
    assert_eq!(show(1.23456.into(), &options), "1");
}

#[test]
fn digits_validation_depends_on_rounding_for_numbers_and_quantities() {
    for mode in [Rounding::Significant, Rounding::Decimals] {
        for digits in [0, 1, 15, 16, 255] {
            let expected = digits <= 15 && (digits > 0 || mode == Rounding::Decimals);
            let mut rules = DisplayRules::default();
            rules.number = NumberDisplay {
                rounding: mode,
                digits,
            };
            assert_eq!(rules.validate().is_ok(), expected);
            if !expected {
                assert_eq!(rules.validate().unwrap_err().key, "error-display-digits");
            }
            rules.number = Default::default();
            let mut rule = QuantityDisplay::default_for(kind("quantity.power").unwrap());
            rule.rounding = mode;
            rule.digits = digits;
            rules.quantities.insert("quantity.power".into(), rule);
            assert_eq!(rules.validate().is_ok(), expected);
        }
    }
}

#[test]
fn legacy_json_defaults_new_fields_and_new_rules_round_trip() {
    let settings: ServerSettings = serde_json::from_str(
        r#"{
        "tick_ms":100,"tick_budget":0,"locale":"en",
        "display":{"quantities":{"quantity.power":{"steps":[{"from":0,"unit":"watt"}],"digits":4}}}
    }"#,
    )
    .unwrap();
    assert_eq!(settings.display.number, NumberDisplay::default());
    assert_eq!(
        settings.display.quantities["quantity.power"].rounding,
        Rounding::Significant
    );
    assert_eq!(settings.display.quantities["quantity.power"].digits, 4);
    let mut settings = settings;
    settings.display.number = NumberDisplay {
        rounding: Rounding::Decimals,
        digits: 0,
    };
    settings
        .display
        .quantities
        .get_mut("quantity.power")
        .unwrap()
        .rounding = Rounding::Decimals;
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"rounding\":\"decimals\""));
    assert_eq!(
        serde_json::from_str::<ServerSettings>(&json).unwrap(),
        settings
    );
    let bytes = postcard::to_allocvec(&settings).unwrap();
    assert_eq!(
        postcard::from_bytes::<ServerSettings>(&bytes).unwrap(),
        settings
    );
}

#[test]
fn rounding_and_digits_share_property_visibility() {
    for ty in [
        DataType::Any,
        DataType::Number,
        DataType::Bool,
        DataType::Named("quantity.power".into()),
        DataType::List(Box::new(DataType::Number)),
    ] {
        assert_eq!(
            crate::to_text::property_visible(&ty, "rounding"),
            crate::to_text::property_visible(&ty, "digits")
        );
    }
}
