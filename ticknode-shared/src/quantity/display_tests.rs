use super::*;

fn custom() -> DisplayRules {
    DisplayRules {
        quantities: [(
            "quantity.power".into(),
            QuantityDisplay {
                digits: 3,
                steps: vec![
                    UnitStep {
                        from: 0.0,
                        unit: "watt".into(),
                    },
                    UnitStep {
                        from: 10000.0,
                        unit: "kilowatt".into(),
                    },
                ],
                rounding: Default::default(),
            },
        )]
        .into(),
        ..Default::default()
    }
}
#[test]
fn default_and_custom_rules_select_after_rounding() {
    let q = kind("quantity.power").unwrap();
    let rule = QuantityDisplay::default_for(q);
    for (v, u) in [
        (0.0, "watt"),
        (1970.0, "kilowatt"),
        (999.7, "kilowatt"),
        (-999.7, "kilowatt"),
        (57.0, "watt"),
    ] {
        assert_eq!(rule.unit(v, q, 3).id, u);
    }
    let rules = custom();
    let rule = rules.effective("quantity.power", q);
    for (v, u) in [
        (5000.0, "watt"),
        (12000.0, "kilowatt"),
        (9997.0, "kilowatt"),
    ] {
        assert_eq!(rule.unit(v, q, 3).id, u);
    }
    let below = QuantityDisplay {
        digits: 3,
        steps: vec![UnitStep {
            from: 10.0,
            unit: "kilowatt".into(),
        }],
        rounding: Default::default(),
    };
    assert_eq!(below.unit(1.0, q, 3).id, "kilowatt");
    assert_eq!(
        crate::to_text::to_text(
            &5000i64.into(),
            &crate::DataType::Named("quantity.power".into()),
            &Default::default(),
            &rules,
            &CanonicalLabels
        ),
        "5000 W"
    );
}
#[test]
fn invalid_rules_have_translatable_errors() {
    let valid = custom();
    valid.validate().unwrap();
    let check = |rules: DisplayRules, key: &str| {
        assert_eq!(
            rules.validate().unwrap_err().key,
            format!("error-display-{key}")
        )
    };
    let mut r = valid.clone();
    r.quantities.insert(
        "foreign.type".into(),
        r.quantities["quantity.power"].clone(),
    );
    check(r, "quantity");
    for digits in [0, 16] {
        let mut r = valid.clone();
        r.quantities.get_mut("quantity.power").unwrap().digits = digits;
        check(r, "digits");
    }
    for count in [0, 9] {
        let mut r = valid.clone();
        r.quantities.get_mut("quantity.power").unwrap().steps = vec![
            UnitStep {
                from: 0.0,
                unit: "watt".into()
            };
            count
        ];
        check(r, "steps");
    }
    let mut r = valid.clone();
    r.quantities.get_mut("quantity.power").unwrap().steps[1].unit = "volt".into();
    check(r, "unit");
    for from in [-1.0, 0.0, f64::NAN, f64::INFINITY] {
        let mut r = valid.clone();
        r.quantities.get_mut("quantity.power").unwrap().steps[1].from = from;
        check(r, "threshold");
    }
    let mut r = valid;
    r.quantities.get_mut("quantity.power").unwrap().steps[0].from = 1.0;
    check(r, "threshold");
}
