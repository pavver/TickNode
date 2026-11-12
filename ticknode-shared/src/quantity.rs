//! Physical values are plain numbers in canonical units; declared types carry their units.

use crate::TypeDefinition;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

mod catalog;
mod catalog_extra;
mod currencies;
mod display;
mod indices;
pub use display::{DisplayRules, NumberDisplay, QuantityDisplay, Rounding, UnitStep};
mod format;
mod parse;
pub use catalog::builtin_quantities;
pub use format::{
    CanonicalLabels, QuantityFormat, QuantityLabels, auto_unit, format_number, format_quantity,
    format_rounded,
};
pub use parse::{literal_text, parse_literal, parse_quantity, unit_in};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantityKind {
    pub canonical: String,
    pub units: Vec<UnitDefinition>,
    /// Affine unit ids, ordered by increasing factor.
    pub auto: Vec<String>,
    /// Default fixed decimal places; absent values use three significant digits.
    #[serde(default)]
    pub decimals: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnitDefinition {
    pub id: String,
    pub symbol: String,
    pub factor: f64,
    pub offset: f64,
    pub aliases: Vec<String>,
    #[serde(default)]
    pub curve: UnitCurve,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitCurve {
    #[default]
    Affine,
    Inverse,
    Log10 {
        scale: f64,
    },
}

/// Converts a number expressed in `unit` to the canonical number stored in DataValue.
pub fn to_canonical(value: f64, unit: &UnitDefinition) -> f64 {
    match unit.curve {
        UnitCurve::Affine => value * unit.factor + unit.offset,
        UnitCurve::Inverse => unit.factor / value,
        UnitCurve::Log10 { scale } => scale * (value * unit.factor).log10() + unit.offset,
    }
}
/// Converts a canonical number to a display or integration unit.
pub fn from_canonical(value: f64, unit: &UnitDefinition) -> f64 {
    match unit.curve {
        UnitCurve::Affine => (value - unit.offset) / unit.factor,
        UnitCurve::Inverse => unit.factor / value,
        UnitCurve::Log10 { scale } => 10f64.powf((value - unit.offset) / scale) / unit.factor,
    }
}

/// Cached catalog for plugin adapters and callers that need borrowed definitions.
pub fn catalog() -> &'static [TypeDefinition] {
    static CATALOG: OnceLock<Vec<TypeDefinition>> = OnceLock::new();
    CATALOG.get_or_init(builtin_quantities)
}

pub fn kind(name: &str) -> Option<&'static QuantityKind> {
    catalog()
        .iter()
        .find(|d| d.name == name)
        .and_then(TypeDefinition::quantity)
}

/// Finds a globally unambiguous symbol/alias (exact case first). When a symbol is also a non-linear
/// unit elsewhere (signal `W`/`mW`), the single linear match wins.
pub fn unit_by_symbol(symbol: &str) -> Option<(&'static str, &'static UnitDefinition)> {
    let symbol = symbol.trim();
    let find = |fold: bool| -> Vec<(&'static str, &'static UnitDefinition)> {
        catalog()
            .iter()
            .flat_map(|d| {
                d.quantity().into_iter().flat_map(move |q| {
                    q.units
                        .iter()
                        .filter(move |u| {
                            std::iter::once(&u.symbol).chain(&u.aliases).any(|s| {
                                if fold {
                                    s.to_lowercase() == symbol.to_lowercase()
                                } else {
                                    s == symbol
                                }
                            })
                        })
                        .map(move |u| (d.name.as_str(), u))
                })
            })
            .collect()
    };
    let mut found = find(false);
    if found.is_empty() {
        found = find(true);
    }
    if found.len() > 1 {
        found.retain(|(_, u)| u.curve == UnitCurve::Affine);
    }
    (found.len() == 1).then(|| found[0])
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod display_tests;

#[cfg(test)]
mod catalog_tests;

#[cfg(test)]
mod rounding_tests;

#[cfg(test)]
mod scales_tests;

#[cfg(test)]
mod curve_tests;
