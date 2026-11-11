//! Lossless editable quantity presentation. Display rounding never changes the stored number.
use ticknode_shared::{*, quantity::*};

pub fn presentation(value: Option<&DataValue>, q: &QuantityKind, preferred: Option<&str>, decimal: &str, rule: &QuantityDisplay) -> (String, String) {
    // A literal (`68 °F`) is shown in its own unit unless another unit is explicitly preferred.
    if let Some(DataValue::String(s)) = value && let Some((n, written)) = parse_literal(s, q) {
        let unit = preferred.and_then(|id| q.units.iter().find(|u| u.id == id)).unwrap_or(written);
        let n = if unit.id == written.id { n } else { from_canonical(to_canonical(n, written), unit) };
        return (n.to_string().replace('.', decimal), unit.id.clone());
    }
    let value = match value { Some(DataValue::Number(n)) => Some(n.to_f64()), _ => None };
    let unit = preferred.and_then(|id| q.units.iter().find(|u| u.id == id))
        .unwrap_or_else(|| rule.unit(value.unwrap_or(0.0), q, rule.digits));
    let number = value.map(|n| from_canonical(n, unit).to_string().replace('.', decimal)).unwrap_or_default();
    (number, unit.id.clone())
}

pub fn parse(input: &str, selected: &str, q: &QuantityKind, title: &str) -> Result<Option<DataValue>, UiMessage> {
    let input = input.trim();
    if input.is_empty() { return Ok(None); }
    let value = if let Some(n) = text::parse_number(input) {
        q.units.iter().find(|u| u.id == selected).map(|u| to_canonical(n.to_f64(), u)).filter(|n| n.is_finite())
    } else { parse_quantity(input, q) };
    value.map(|n| Some(DataValue::from(n))).ok_or_else(|| {
        UiMessage::new("parse-error-quantity", format!("\"{input}\" is not a physical quantity in {}", q.canonical))
            .arg("value", input).arg("type", title)
    })
}

/// Literal mode: a bare number takes the selected unit, an explicit suffix overrides it; the result is
/// `<number> <catalog symbol>` text and the unit id to keep selected.
pub fn parse_literal_input(input: &str, selected: &str, q: &QuantityKind, title: &str) -> Result<Option<(DataValue, String)>, UiMessage> {
    let input = input.trim();
    if input.is_empty() { return Ok(None); }
    let found = match text::parse_number(input) {
        Some(n) => q.units.iter().find(|u| u.id == selected).map(|u| (n.to_f64(), u)),
        None => parse_literal(input, q),
    };
    found.filter(|(n, _)| n.is_finite())
        .map(|(n, u)| Some((DataValue::String(literal_text(n, u)), u.id.clone())))
        .ok_or_else(|| {
            UiMessage::new("parse-error-quantity", format!("\"{input}\" is not a physical quantity in {}", q.canonical))
                .arg("value", input).arg("type", title)
        })
}

#[cfg(test)]
mod literal_tests {
    use super::*;
    #[test]
    fn literals_keep_the_written_unit() {
        let q = kind("quantity.temperature").unwrap();
        let rule = QuantityDisplay::default_for(q);
        assert_eq!(parse_literal_input("68", "fahrenheit", q, "T"), Ok(Some(("68 °F".into(), "fahrenheit".into()))));
        assert_eq!(parse_literal_input("20,5 °C", "fahrenheit", q, "T"), Ok(Some(("20.5 °C".into(), "celsius".into()))));
        assert!(parse_literal_input("5 V", "celsius", q, "T").is_err());
        assert_eq!(presentation(Some(&"68 °F".into()), q, None, ",", &rule), ("68".into(), "fahrenheit".into()));
        // Legacy canonical numbers still follow the project rule.
        assert_eq!(presentation(Some(&20i64.into()), q, None, ".", &rule), ("20".into(), "celsius".into()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn units_commit_canonical_values_and_explicit_suffix_overrides_selection() {
        let q = kind("quantity.power").unwrap();
        assert_eq!(parse("2,5", "kilowatt", q, "Power"), Ok(Some(2500i64.into())));
        assert_eq!(parse("2 W", "kilowatt", q, "Power"), Ok(Some(2i64.into())));
        assert_eq!(parse("", "watt", q, "Power"), Ok(None));
        assert!(parse("5 V", "watt", q, "Power").is_err());
        let error = parse("invalid", "watt", q, "Потужність").unwrap_err();
        assert!(error.fallback.contains("physical quantity"));
        assert!(!error.fallback.contains("Потужність"));
        assert!(error.args.contains(&("type".into(), "Потужність".into())));
        let value = DataValue::from(1970.123456789);
        let (text, unit) = presentation(Some(&value), q, None, ",", &QuantityDisplay::default_for(q));
        assert_eq!(unit, "kilowatt");
        assert_eq!(parse(&text, &unit, q, "Power"), Ok(Some(value)));
    }
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    #[test]
    fn initial_unit_uses_project_rules_without_rounding_the_editable_value() {
        let q = kind("quantity.power").unwrap();
        let rule = QuantityDisplay { digits: 3, steps: vec![
            UnitStep { from: 0.0, unit: "watt".into() }, UnitStep { from: 10000.0, unit: "kilowatt".into() },
        ], rounding: Default::default(), };
        assert_eq!(presentation(Some(&5000i64.into()), q, None, ".", &rule), ("5000".into(), "watt".into()));
        assert_eq!(presentation(Some(&12000i64.into()), q, None, ".", &rule), ("12".into(), "kilowatt".into()));
    }
}
