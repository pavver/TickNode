use super::{QuantityKind, Rounding, UnitDefinition, from_canonical};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantityFormat {
    pub unit: Option<String>,
    pub digits: u8,
    pub rounding: Rounding,
}
impl Default for QuantityFormat {
    fn default() -> Self {
        Self {
            unit: None,
            digits: 3,
            rounding: Rounding::Significant,
        }
    }
}

pub trait QuantityLabels {
    fn unit_symbol(&self, unit_id: &str) -> Option<String>;
    fn decimal_separator(&self) -> String;
    fn bool_text(&self, value: bool) -> String {
        value.to_string()
    }
    fn duration_unit(&self, unit: &str) -> String {
        unit.into()
    }
}

/// Locale-independent fallback (catalog symbols and a decimal point).
pub struct CanonicalLabels;
impl QuantityLabels for CanonicalLabels {
    fn unit_symbol(&self, _: &str) -> Option<String> {
        None
    }
    fn decimal_separator(&self) -> String {
        ".".into()
    }
}

fn decimals(value: f64, digits: u8) -> usize {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    (i32::from(digits.clamp(1, 15)) - 1 - value.abs().log10().floor() as i32).clamp(0, 340) as usize
}

pub fn auto_unit(value: f64, kind: &QuantityKind, digits: u8) -> &UnitDefinition {
    super::QuantityDisplay::default_for(kind).unit(value, kind, digits)
}

pub fn format_number(value: f64, digits: u8, labels: &dyn QuantityLabels) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let mut number = format!("{:.*}", decimals(value, digits), value);
    if number.contains('.') {
        number = number
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
    }
    if number == "-0" {
        number = "0".into();
    }
    number.replace('.', &labels.decimal_separator())
}

/// Fixed decimals preserve trailing zeros; significant digits retain the existing presentation.
pub fn format_rounded(
    value: f64,
    rounding: Rounding,
    digits: u8,
    labels: &dyn QuantityLabels,
) -> String {
    if !value.is_finite() {
        return String::new();
    }
    if rounding == Rounding::Significant {
        return format_number(value, digits, labels);
    }
    let mut number = format!("{:.*}", usize::from(digits.min(15)), value);
    if number.starts_with('-') && number[1..].chars().all(|c| c == '0' || c == '.') {
        number.remove(0);
    }
    number.replace('.', &labels.decimal_separator())
}

pub fn format_quantity(
    value: f64,
    kind: &QuantityKind,
    format: &QuantityFormat,
    rule: &super::QuantityDisplay,
    labels: &dyn QuantityLabels,
) -> String {
    let mut rule = rule.clone();
    rule.rounding = format.rounding;
    let unit = format
        .unit
        .as_ref()
        .and_then(|id| kind.units.iter().find(|u| &u.id == id))
        .unwrap_or_else(|| rule.unit(value, kind, format.digits));
    let symbol = labels
        .unit_symbol(&unit.id)
        .unwrap_or_else(|| unit.symbol.clone());
    let converted = from_canonical(value, unit);
    if !value.is_finite() || !converted.is_finite() {
        return String::new();
    }
    format!(
        "{} {symbol}",
        format_rounded(converted, format.rounding, format.digits, labels)
    )
}
