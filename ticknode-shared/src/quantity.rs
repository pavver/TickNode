//! Physical values are plain numbers in canonical units; declared types carry their units.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use crate::{TypeDefinition, TypeKind};

mod catalog;
mod catalog_extra;
mod display;
pub use display::{DisplayRules, QuantityDisplay, UnitStep};
mod format;
mod parse;
pub use catalog::builtin_quantities;
pub use format::{CanonicalLabels,QuantityFormat, QuantityLabels, auto_unit, format_number, format_quantity};
pub use parse::{parse_quantity, unit_in};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantityKind {
    pub canonical: String,
    pub units: Vec<UnitDefinition>,
    /// Unit ids, ordered by increasing factor.
    pub auto: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnitDefinition {
    pub id: String,
    pub symbol: String,
    pub factor: f64,
    pub offset: f64,
    pub aliases: Vec<String>,
}

/// Converts a number expressed in `unit` to the canonical number stored in DataValue.
pub fn to_canonical(value: f64, unit: &UnitDefinition) -> f64 { value * unit.factor + unit.offset }
/// Converts a canonical number to a display or integration unit.
pub fn from_canonical(value: f64, unit: &UnitDefinition) -> f64 { (value - unit.offset) / unit.factor }

/// Cached catalog for plugin adapters and callers that need borrowed definitions.
pub fn catalog() -> &'static [TypeDefinition] {
    static CATALOG: OnceLock<Vec<TypeDefinition>> = OnceLock::new();
    CATALOG.get_or_init(builtin_quantities)
}

pub fn kind(name: &str) -> Option<&'static QuantityKind> {
    catalog().iter().find(|d| d.name == name).and_then(TypeDefinition::quantity)
}

/// Finds a globally unambiguous symbol/alias (exact case first).
pub fn unit_by_symbol(symbol: &str) -> Option<(&'static str, &'static UnitDefinition)> {
    let find = |fold: bool| {
        let mut matches = catalog().iter().filter_map(|d| {
            let TypeKind::Quantity(q) = &d.kind else { return None };
            unit_in(q, symbol, fold).map(|u| (d.name.as_str(), u))
        });
        let found = matches.next()?;
        matches.next().is_none().then_some(found)
    };
    find(false).or_else(|| find(true))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod display_tests;

#[cfg(test)]
mod catalog_tests;
