//! Quantity display uses UI locale, independently of server formatting language.
use ticknode_shared::{*, quantity::QuantityLabels};
use leptos::prelude::*;
use std::cell::RefCell;

thread_local! {
    static DISPLAY: RefCell<DisplayRules> = RefCell::new(DisplayRules::default());
}
pub fn set_display_rules(rules: &DisplayRules) {
    DISPLAY.with(|d| *d.borrow_mut() = rules.clone());
    super::TRANSLATIONS_VERSION.with(|c| { if let Some(s) = c.get() { s.try_update(|n| *n += 1); } });
}
pub fn display_rules() -> DisplayRules {
    super::TRANSLATIONS_VERSION.with(|c| { if let Some(s) = c.get() { let _ = s.try_get(); } });
    DISPLAY.with(|d| d.borrow().clone())
}
use super::{lookup, TYPES};

pub struct UiQuantityLabels;
impl QuantityLabels for UiQuantityLabels {
    fn unit_symbol(&self, id: &str) -> Option<String> { unit_symbol(id) }
    fn decimal_separator(&self) -> String { decimal_separator() }
    fn bool_text(&self, value: bool) -> String {
        lookup(if value { "text-true" } else { "text-false" }).unwrap_or_else(|| value.to_string())
    }
    fn duration_unit(&self, unit: &str) -> String {
        lookup(&format!("text-duration-{unit}")).unwrap_or_else(|| unit.into())
    }
}

/// The same presentation as the To text node, with the UI locale, the project display rules and the
/// browser time zone (wire value labels).
pub fn value_as_text(value: &DataValue, declared: &DataType) -> String {
    let rules = display_rules();
    TYPES.with(|types| to_text::to_text_with_identifiers(value, declared, &Default::default(), &rules, &UiQuantityLabels, &types.borrow(),
        &|name: &str, id: i64| super::identifier_label(name, id).map(|label| format!("{label} ({id})"))))
}
pub fn unit_symbol(id: &str) -> Option<String> { lookup(&format!("unit-{}", fluent_slug(id))) }
pub fn decimal_separator() -> String { lookup("number-decimal-separator").unwrap_or_else(|| ".".into()) }

/// Declared structure fields retain quantity types even though their values are plain numbers.
pub fn declared_field_type(t: &DataType, name: &str) -> Option<DataType> {
    TYPES.with(|types| types.borrow().definition_of(t)?.fields().iter()
        .find(|f| f.name == name).map(|f| f.data_type.clone()))
}

pub fn quantity_text(value: &DataValue, t: &DataType) -> Option<String> {
    let DataValue::Number(n) = value else { return None };
    TYPES.with(|types| {
        let types = types.borrow();
        let q = types.definition_of(t)?.quantity()?;
        Some(to_text::quantity_text(n.to_f64(), &data_type_name(t), q, &Default::default(), &display_rules(), &UiQuantityLabels))
    })
}

pub fn format_match_case_in(case: &condition::MatchCase, inputs: &[condition::MatchInput]) -> String {
    use condition::Operand;
    // Reuse the ordinary presentation for operators, joins, strings and other value types.
    let mut shown = case.clone();
    for c in &mut shown.conditions {
        if let Operand::Input(name) = &c.left
            && let Some(t) = inputs.iter().find(|i| &i.name == name).map(|i| &i.value_type)
            && let Some(Operand::Value(v)) = &c.right
        {
            let literal = match v {
                DataValue::String(s) => TYPES.with(|types| types.borrow().definition_of(t)?.quantity()
                    .and_then(|q| quantity::parse_literal(s, q)).map(|(n, u)| {
                        let symbol = unit_symbol(&u.id).unwrap_or_else(|| u.symbol.clone());
                        format!("{} {symbol}", n.to_string().replace('.', &decimal_separator()))
                    })),
                _ => None,
            };
            let canonical = TYPES.with(|types| text::coerce_to_in(&types.borrow(), v.clone(), t));
            if let Some(label) = literal.or_else(|| quantity_text(&canonical, t)).or_else(|| super::identifier_text(&canonical, t)) {
                // Input operands display without quotation marks; this temporary value never persists.
                c.right = Some(Operand::Input(label));
            }
        }
    }
    super::format_match_case(&shown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Locale, with_locale, set_runtime_translations, set_known_types};
    #[test]
    fn physical_values_and_match_constants_follow_ui_locale() {
        let resources = vec![
            ("en".into(), include_str!("../../../ticknode-core/locales/en.ftl").into()),
            ("uk".into(), include_str!("../../../ticknode-core/locales/uk.ftl").into()),
        ];
        set_runtime_translations(&resources);
        set_known_types(&TypeRegistry::from_definitions(quantity::builtin_quantities()));
        let power = DataType::Named("quantity.power".into());
        let inputs = vec![condition::MatchInput::parse("1:p:quantity.power").unwrap()];
        let case = condition::MatchCase::parse("1: p > 2 kW").unwrap();
        for (locale, number, label) in [(Locale::En, "1.97 kW", "p > 2 kW"), (Locale::Uk, "1,97 кВт", "p > 2 кВт")] {
            with_locale(locale, || {
                assert_eq!(quantity_text(&1970i64.into(), &power).as_deref(), Some(number));
                assert_eq!(format_match_case_in(&case, &inputs), label);
                assert_eq!(crate::panels::display_value_in(&1970i64.into(), &power), number);
            });
        }
        set_runtime_translations(&[]);
        set_known_types(&TypeRegistry::new());
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    #[test]
    fn project_rules_and_error_keys_are_available_in_both_locales() {
        use crate::i18n::{Locale, with_locale, set_runtime_translations, set_known_types};
        set_runtime_translations(&[("en".into(), include_str!("../../../ticknode-core/locales/en.ftl").into()),
            ("uk".into(), include_str!("../../../ticknode-core/locales/uk.ftl").into())]);
        set_known_types(&TypeRegistry::from_definitions(quantity::builtin_quantities()));
        set_display_rules(&DisplayRules { quantities: [("quantity.power".into(), QuantityDisplay {
            digits: 3, steps: vec![UnitStep { from: 0.0, unit: "watt".into() }],
        rounding: Default::default(),
    })].into(), ..Default::default() });
        for (locale, expected) in [(Locale::En, "5000 W"), (Locale::Uk, "5000 Вт")] {
            with_locale(locale, || {
                assert_eq!(quantity_text(&5000i64.into(), &DataType::Named("quantity.power".into())).as_deref(), Some(expected));
                for reason in ["quantity", "unit", "steps", "digits", "threshold"] {
                    assert!(lookup(&format!("error-display-{reason}")).is_some());
                }
            });
        }
        set_display_rules(&Default::default());
        set_runtime_translations(&[]);
        set_known_types(&TypeRegistry::new());
    }
}

#[cfg(test)]
mod structure_tests {
    use super::*;
    #[test]
    fn nested_quantity_values_keep_their_rules_in_inspector_text() {
        use crate::i18n::{Locale, with_locale, set_known_types, set_runtime_translations};
        let mut definitions = quantity::builtin_quantities();
        definitions.push(TypeDefinition::new_struct("test.meter", "Meter", vec![
            FieldDefinition::new("value", "Value", DataType::Named("quantity.power".into())),
        ]));
        set_known_types(&TypeRegistry::from_definitions(definitions));
        set_runtime_translations(&[("en".into(), include_str!("../../../ticknode-core/locales/en.ftl").into())]);
        let value = DataValue::Object { type_name: Some("test.meter".into()), fields: [
            ("value".into(), ObjectField { name: "Value".into(), description: String::new(), value: 5000i64.into() }),
        ].into() };
        set_display_rules(&DisplayRules { quantities: [("quantity.power".into(), QuantityDisplay {
            digits: 3, steps: vec![UnitStep { from: 0.0, unit: "watt".into() }],
        rounding: Default::default(),
    })].into(), ..Default::default() });
        with_locale(Locale::En, || assert_eq!(crate::panels::display_value_in(&value, &DataType::Any), "{value: 5000 W}"));
        set_display_rules(&Default::default());
        set_known_types(&TypeRegistry::new());
        set_runtime_translations(&[]);
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;
    use crate::i18n::{Locale, with_locale, set_runtime_translations};
    #[test]
    fn wire_values_use_to_text_presentation_in_ui_locale() {
        set_runtime_translations(&[("en".into(), include_str!("../../../ticknode-core/locales/en.ftl").into()),
            ("uk".into(), include_str!("../../../ticknode-core/locales/uk.ftl").into())]);
        with_locale(Locale::Uk, || {
            assert_eq!(value_as_text(&DataValue::Number(NumberValue::Float(1.23456)), &DataType::Number), "1,23");
            assert_ne!(value_as_text(&DataValue::Bool(true), &DataType::Bool), "true");
            assert_eq!(value_as_text(&DataValue::String("x".into()), &DataType::String), "x");
        });
        set_runtime_translations(&[]);
    }
}

#[cfg(test)]
mod rounding_tests {
    use super::*;
    #[test]
    fn decimals_reach_wire_and_inspector_labels_in_ui_locale() {
        use crate::i18n::{Locale, with_locale, set_known_types, set_runtime_translations};
        set_known_types(&TypeRegistry::from_definitions(quantity::builtin_quantities()));
        set_runtime_translations(&[
            ("en".into(), include_str!("../../../ticknode-core/locales/en.ftl").into()),
            ("uk".into(), include_str!("../../../ticknode-core/locales/uk.ftl").into()),
        ]);
        let ty = DataType::Named("quantity.power".into());
        let mut rule = QuantityDisplay::default_for(quantity::kind("quantity.power").unwrap());
        rule.rounding = Rounding::Decimals;
        rule.digits = 2;
        set_display_rules(&DisplayRules {
            quantities: [("quantity.power".into(), rule)].into(),
            number: NumberDisplay { rounding: Rounding::Decimals, digits: 2 },
        });
        for (locale, quantity, number) in [(Locale::En, "999.70 W", "23.00"), (Locale::Uk, "999,70 Вт", "23,00")] {
            with_locale(locale, || {
                assert_eq!(value_as_text(&999.7.into(), &ty), quantity);
                assert_eq!(crate::panels::display_value_in(&999.7.into(), &ty), quantity);
                assert_eq!(value_as_text(&23i64.into(), &DataType::Number), number);
            });
        }
        set_display_rules(&Default::default());
        set_known_types(&TypeRegistry::new());
        set_runtime_translations(&[]);
    }
}
