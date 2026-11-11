//! Server-side quantity labels from simple embedded Fluent messages, cached per locale.
use std::{collections::BTreeMap, sync::OnceLock};
use fluent_syntax::{ast::{Entry, PatternElement}, parser};
use ticknode_shared::{fluent_slug, quantity::QuantityLabels};

pub struct Labels(BTreeMap<String, String>);
impl QuantityLabels for Labels {
    fn unit_symbol(&self, id: &str) -> Option<String> { self.0.get(&format!("unit-{}", fluent_slug(id))).cloned() }
    fn bool_text(&self, value: bool) -> String {
        self.0.get(if value { "text-true" } else { "text-false" }).cloned().unwrap_or_else(|| value.to_string())
    }
    fn duration_unit(&self, unit: &str) -> String {
        self.0.get(&format!("text-duration-{unit}")).cloned().unwrap_or_else(|| unit.into())
    }
    fn decimal_separator(&self) -> String { self.0.get("number-decimal-separator").cloned().unwrap_or_else(|| ".".into()) }
}

pub fn labels(locale: &str) -> &'static Labels {
    static EN: OnceLock<Labels> = OnceLock::new();
    static UK: OnceLock<Labels> = OnceLock::new();
    let (cache, source) = if locale == "uk" { (&UK, crate::translations::CORE_TRANSLATIONS[1].1) }
        else { (&EN, crate::translations::CORE_TRANSLATIONS[0].1) };
    cache.get_or_init(|| {
        let resource = parser::parse(source).expect("valid core translations");
        Labels(resource.body.into_iter().filter_map(|e| {
            let Entry::Message(m) = e else { return None };
            let value = m.value?.elements.into_iter().map(|e| match e {
                PatternElement::TextElement { value } => Some(value),
                PatternElement::Placeable { expression: fluent_syntax::ast::Expression::Inline(fluent_syntax::ast::InlineExpression::StringLiteral { value }) } => Some(value),
                _ => None,
            }).collect::<Option<String>>()?;
            Some((m.id.name.into(), value))
        }).collect())
    })
}
