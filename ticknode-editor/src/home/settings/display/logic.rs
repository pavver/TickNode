//! Lossless canonical thresholds with editable numbers in each step's selected unit.
use std::collections::BTreeMap;
use ticknode_shared::{
    quantity::{self, QuantityKind, from_canonical, to_canonical},
    *,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StepDraft {
    pub threshold: String,
    pub unit: String,
    original: (String, String, f64),
}
impl StepDraft {
    fn new(from: f64, unit: &quantity::UnitDefinition) -> Self {
        let converted = from_canonical(from, unit);
        let threshold = if converted.is_finite() {
            converted.to_string()
        } else {
            String::new()
        };
        Self {
            original: (threshold.clone(), unit.id.clone(), from),
            threshold,
            unit: unit.id.clone(),
        }
    }
    fn canonical(&self, q: &QuantityKind) -> Option<f64> {
        if self.threshold == self.original.0 && self.unit == self.original.1 {
            return Some(self.original.2);
        }
        let unit = q.units.iter().find(|u| u.id == self.unit)?;
        text::parse_number(&self.threshold)
            .map(|v| to_canonical(v.to_f64(), unit))
            .filter(|v| v.is_finite())
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct RuleDraft {
    pub steps: Vec<StepDraft>,
    pub digits: String,
    pub rounding: Rounding,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DisplayDraft {
    pub rules: BTreeMap<String, RuleDraft>,
    pub number: NumberDraft,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberDraft {
    pub rounding: Rounding,
    pub digits: String,
}
impl Default for NumberDraft {
    fn default() -> Self {
        Self::new(NumberDisplay::default())
    }
}
impl NumberDraft {
    fn new(rule: NumberDisplay) -> Self {
        Self {
            rounding: rule.rounding,
            digits: rule.digits.to_string(),
        }
    }
    fn parse(&self) -> Option<NumberDisplay> {
        Some(NumberDisplay {
            rounding: self.rounding,
            digits: self.digits.parse().ok()?,
        })
    }
}

impl RuleDraft {
    pub fn new(rule: &QuantityDisplay, q: &QuantityKind) -> Self {
        Self {
            digits: rule.digits.to_string(),
            rounding: rule.rounding,
            steps: rule
                .steps
                .iter()
                .filter_map(|s| {
                    q.units
                        .iter()
                        .find(|u| u.id == s.unit)
                        .map(|u| StepDraft::new(s.from, u))
                })
                .collect(),
        }
    }
    pub fn parse(&self, q: &QuantityKind) -> Option<QuantityDisplay> {
        let steps = self
            .steps
            .iter()
            .map(|s| {
                Some(UnitStep {
                    from: s.canonical(q)?,
                    unit: s.unit.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(QuantityDisplay {
            steps,
            digits: self.digits.parse().ok()?,
            rounding: self.rounding,
        })
    }
    pub fn change_unit(&mut self, index: usize, id: String, q: &QuantityKind) {
        let Some(step) = self.steps.get_mut(index) else {
            return;
        };
        let canonical = step.canonical(q);
        if let Some(unit) = q.units.iter().find(|u| u.id == id) {
            if let Some(value) = canonical {
                *step = StepDraft::new(value, unit);
            } else {
                step.unit = id;
            }
        }
    }
    pub fn add_step(&mut self, q: &QuantityKind) {
        if self.steps.len() >= 8 {
            return;
        }
        let Some(mut rule) = self.parse(q) else {
            return;
        };
        let from = rule
            .steps
            .last()
            .map_or(1.0, |s| if s.from > 0.0 { s.from * 10.0 } else { 1.0 });
        let unit = rule
            .steps
            .last()
            .map_or_else(|| q.canonical.clone(), |s| s.unit.clone());
        rule.steps.push(UnitStep { from, unit });
        *self = Self::new(&rule, q);
    }
}
impl DisplayDraft {
    pub fn new(rules: &DisplayRules) -> Self {
        Self {
            number: NumberDraft::new(rules.number),
            rules: rules
                .quantities
                .iter()
                .filter_map(|(name, rule)| {
                    Some((name.clone(), RuleDraft::new(rule, quantity::kind(name)?)))
                })
                .collect(),
        }
    }
    pub fn effective(&self, name: &str) -> RuleDraft {
        self.rules.get(name).cloned().unwrap_or_else(|| {
            let q = quantity::kind(name).expect("catalog quantity");
            RuleDraft::new(&QuantityDisplay::default_for(q), q)
        })
    }
    pub fn edit(&mut self, name: &str, edit: impl FnOnce(&mut RuleDraft, &QuantityKind)) {
        let Some(q) = quantity::kind(name) else {
            return;
        };
        let rule = self
            .rules
            .entry(name.into())
            .or_insert_with(|| RuleDraft::new(&QuantityDisplay::default_for(q), q));
        edit(rule, q);
    }
    pub fn parse(&self) -> Option<DisplayRules> {
        let quantities = self
            .rules
            .iter()
            .map(|(name, rule)| Some((name.clone(), rule.parse(quantity::kind(name)?)?)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        let rules = DisplayRules {
            quantities,
            number: self.number.parse()?,
        };
        rules.validate().ok()?;
        Some(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_thresholds_preserve_canonical_values_and_validate() {
        let mut draft = DisplayDraft::default();
        draft.edit("quantity.power", |r, q| {
            r.steps.truncate(2);
            r.steps[1].threshold = "10".into();
            r.change_unit(1, "watt".into(), q);
            assert_eq!(r.steps[1].threshold, "10000");
        });
        let rules = draft.parse().unwrap();
        assert_eq!(rules.quantities["quantity.power"].steps[1].from, 10000.0);
        assert_eq!(DisplayDraft::new(&rules).parse(), draft.parse());
        draft.edit("quantity.power", |r, _| {
            r.steps[1].threshold = "invalid".into()
        });
        assert!(draft.parse().is_none());
        draft.rules.remove("quantity.power");
        assert_eq!(draft.parse(), Some(DisplayRules::default()));
    }
    #[test]
    fn offset_units_and_step_limits() {
        let mut draft = DisplayDraft::default();
        draft.edit("quantity.temperature", |r, q| {
            r.change_unit(0, "fahrenheit".into(), q);
            assert_eq!(r.parse(q).unwrap().steps[0].from, 0.0);
            for _ in 0..10 {
                r.add_step(q);
            }
            assert_eq!(r.steps.len(), 8);
        });
        assert!(draft.parse().is_some());
    }
}

#[cfg(test)]
mod rounding_tests {
    use super::*;
    #[test]
    fn number_and_quantity_rounding_validate_round_trip_and_reset() {
        let mut draft = DisplayDraft::default();
        draft.number.digits = "0".into();
        assert!(draft.parse().is_none());
        draft.number.rounding = Rounding::Decimals;
        draft.edit("quantity.power", |r, _| {
            r.rounding = Rounding::Decimals;
            r.digits = "0".into();
        });
        let rules = draft.parse().unwrap();
        assert_eq!(rules.number.digits, 0);
        assert_eq!(
            rules.quantities["quantity.power"].rounding,
            Rounding::Decimals
        );
        assert_eq!(DisplayDraft::new(&rules), draft);
        draft.edit("quantity.power", |r, _| r.rounding = Rounding::Significant);
        assert!(draft.parse().is_none());
        draft.rules.clear();
        for digits in ["16", "-1", "abc", ""] {
            draft.number.digits = digits.into();
            assert!(draft.parse().is_none());
        }
        draft.number = Default::default();
        assert_eq!(draft.parse(), Some(DisplayRules::default()));
    }
}
