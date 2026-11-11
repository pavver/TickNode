//! Project display preferences. Canonical values and graph persistence remain unchanged.
use super::{QuantityKind, UnitDefinition, kind};
use crate::UiMessage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DisplayRules {
    pub quantities: BTreeMap<String, QuantityDisplay>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantityDisplay {
    pub steps: Vec<UnitStep>,
    pub digits: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitStep {
    pub from: f64,
    pub unit: String,
}

impl QuantityDisplay {
    pub fn default_for(kind: &QuantityKind) -> Self {
        let mut steps: Vec<_> = kind
            .auto
            .iter()
            .filter_map(|id| {
                kind.units.iter().find(|u| &u.id == id).map(|u| UnitStep {
                    from: u.factor,
                    unit: id.clone(),
                })
            })
            .collect();
        if steps.is_empty() {
            steps.push(UnitStep {
                from: 0.0,
                unit: kind.canonical.clone(),
            });
        }
        steps[0].from = 0.0;
        Self { steps, digits: 3 }
    }

    /// Select using rounded magnitude, including a carry across a step boundary.
    pub fn unit<'a>(&self, value: f64, kind: &'a QuantityKind, digits: u8) -> &'a UnitDefinition {
        let magnitude = value.abs();
        let rounded = if magnitude > 0.0 && magnitude.is_finite() {
            let scale =
                10f64.powf(magnitude.log10().floor() - f64::from(digits.clamp(1, 15)) + 1.0);
            if scale.is_finite() && scale > 0.0 {
                (magnitude / scale).round() * scale
            } else {
                magnitude
            }
        } else {
            magnitude
        };
        self.steps
            .iter()
            .rev()
            .find(|s| s.from <= rounded)
            .or_else(|| self.steps.first())
            .and_then(|s| kind.units.iter().find(|u| u.id == s.unit))
            .unwrap_or_else(|| {
                kind.units
                    .iter()
                    .find(|u| u.id == kind.canonical)
                    .unwrap_or(&kind.units[0])
            })
    }
}

impl DisplayRules {
    pub fn effective(&self, name: &str, kind: &QuantityKind) -> QuantityDisplay {
        self.quantities
            .get(name)
            .cloned()
            .unwrap_or_else(|| QuantityDisplay::default_for(kind))
    }

    pub fn validate(&self) -> Result<(), UiMessage> {
        for (name, rule) in &self.quantities {
            let invalid = |reason: &str| {
                UiMessage::new(
                    &format!("error-display-{reason}"),
                    format!("Invalid display rule for {name}: {reason}"),
                )
                .arg("quantity", name)
            };
            let Some(q) = kind(name) else {
                return Err(invalid("quantity"));
            };
            if !(1..=15).contains(&rule.digits) {
                return Err(invalid("digits"));
            }
            if !(1..=8).contains(&rule.steps.len()) {
                return Err(invalid("steps"));
            }
            for (i, step) in rule.steps.iter().enumerate() {
                if !q.units.iter().any(|u| u.id == step.unit) {
                    return Err(invalid("unit"));
                }
                if !step.from.is_finite()
                    || step.from < 0.0
                    || (i == 0 && step.from != 0.0)
                    || (i > 0 && step.from <= rule.steps[i - 1].from)
                {
                    return Err(invalid("threshold"));
                }
            }
        }
        Ok(())
    }
}
