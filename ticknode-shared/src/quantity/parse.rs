use super::{QuantityKind, UnitDefinition, to_canonical};

/// Case folding is accepted only when it identifies exactly one unit.
pub fn unit_in<'a>(kind: &'a QuantityKind, symbol: &str, fold: bool) -> Option<&'a UnitDefinition> {
    let wanted = symbol.trim();
    let mut matches = kind.units.iter().filter(|u| {
        std::iter::once(&u.symbol).chain(&u.aliases).any(|s| {
            if fold { s.to_lowercase() == wanted.to_lowercase() } else { s == wanted }
        })
    });
    let found = matches.next()?;
    matches.next().is_none().then_some(found)
}

/// Bare numbers are canonical; explicit units must belong to this quantity.
pub fn parse_quantity(text: &str, kind: &QuantityKind) -> Option<f64> {
    let text = text.trim();
    if let Some(n) = crate::text::parse_number(text) { return Some(n.to_f64()); }
    // Trying character boundaries also handles exponent notation and units containing spaces.
    for (i, _) in text.char_indices().rev() {
        let Some(n) = crate::text::parse_number(&text[..i]) else { continue };
        let suffix = text[i..].trim();
        let Some(unit) = unit_in(kind, suffix, false).or_else(|| unit_in(kind, suffix, true)) else { continue };
        let canonical = to_canonical(n.to_f64(), unit);
        return canonical.is_finite().then_some(canonical);
    }
    None
}
