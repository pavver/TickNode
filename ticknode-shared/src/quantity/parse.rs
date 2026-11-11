use super::{QuantityKind, UnitDefinition, to_canonical};

/// Case folding is accepted only when it identifies exactly one unit.
pub fn unit_in<'a>(kind: &'a QuantityKind, symbol: &str, fold: bool) -> Option<&'a UnitDefinition> {
    let wanted = symbol.trim();
    let mut matches = kind.units.iter().filter(|u| {
        std::iter::once(&u.symbol).chain(&u.aliases).any(|s| {
            if fold {
                s.to_lowercase() == wanted.to_lowercase()
            } else {
                s == wanted
            }
        })
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

/// Bare numbers are canonical; explicit units must belong to this quantity.
pub fn parse_quantity(text: &str, kind: &QuantityKind) -> Option<f64> {
    if let Some(n) = crate::text::parse_number(text.trim()) {
        return Some(n.to_f64());
    }
    let (n, unit) = parse_literal(text, kind)?;
    let canonical = to_canonical(n, unit);
    canonical.is_finite().then_some(canonical)
}

/// A number with an explicit unit of this quantity (`68 °F`), keeping the written unit.
pub fn parse_literal<'a>(text: &str, kind: &'a QuantityKind) -> Option<(f64, &'a UnitDefinition)> {
    let text = text.trim();
    // Trying character boundaries also handles exponent notation and units containing spaces.
    for (i, _) in text.char_indices().rev() {
        let Some(n) = crate::text::parse_number(&text[..i]) else {
            continue;
        };
        let suffix = text[i..].trim();
        let Some(unit) = unit_in(kind, suffix, false).or_else(|| unit_in(kind, suffix, true))
        else {
            continue;
        };
        return to_canonical(n.to_f64(), unit)
            .is_finite()
            .then_some((n.to_f64(), unit));
    }
    None
}

/// Canonical literal text (`68 °F`): catalog symbol and a decimal point, as stored in conditions.
pub fn literal_text(value: f64, unit: &UnitDefinition) -> String {
    format!("{value} {}", unit.symbol)
}
