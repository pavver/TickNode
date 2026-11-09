//! Translation resources: the core's own (`locales/*.ftl`) and those shipped by plugins.
//!
//! Every resource is Mozilla Fluent source for one locale. The editor merges what the server sends
//! into one bundle, so plugin resources are validated here: syntax errors reject the whole resource,
//! and a message whose id does not start with one of the plugin's own prefixes is dropped.
//! Key conventions are described in the plugin SDK docs (`ticknode-plugin-sdk`).

use std::collections::{BTreeMap, HashSet};

use fluent_syntax::ast::{Entry, Resource};
use fluent_syntax::{parser, serializer};
use ticknode_shared::*;
use tracing::warn;

/// Translations of the built-in nodes (`node-<type slug>-title|description|help|pin-<pin>`).
pub const CORE_TRANSLATIONS: [(&str, &str); 2] = [
    ("en", include_str!("../locales/en.ftl")),
    ("uk", include_str!("../locales/uk.ftl")),
];

/// Which Fluent ids a plugin is allowed to define.
#[derive(Debug, Clone, Default)]
pub struct AllowedIds {
    /// `node-<slug>` for each node type of the plugin.
    nodes: Vec<String>,
    /// `type-<slug>` for each data type of the plugin.
    types: Vec<String>,
    /// `<plugin slug>`: free keys are `<slug>-...`, settings are `setting-<slug>-...`.
    plugin: String,
}

impl AllowedIds {
    pub fn new(plugin: &str, node_types: &[String], data_types: &[String]) -> Self {
        Self {
            nodes: node_types.iter().map(|t| format!("node-{}", fluent_slug(t))).collect(),
            types: data_types.iter().map(|t| format!("type-{}", fluent_slug(t))).collect(),
            plugin: fluent_slug(plugin),
        }
    }

    /// Prefix that plugin-defined UI error keys must start with.
    pub fn free_prefix(&self) -> String {
        format!("{}-", self.plugin)
    }

    pub fn message_allowed(&self, id: &str) -> bool {
        let under = |base: &str, suffixes: &[&str], prefixes: &[&str]| {
            id.strip_prefix(base).and_then(|r| r.strip_prefix('-')).is_some_and(|rest| {
                suffixes.contains(&rest) || prefixes.iter().any(|p| rest.starts_with(p) && rest.len() > p.len())
            })
        };
        id.starts_with("category-")
            || id.starts_with(&self.free_prefix())
            || id.starts_with(&format!("setting-{}-", self.plugin))
            || self.nodes.iter().any(|n| under(n, &["title", "description", "help"], &["pin-", "prop-"]))
            || self.types.iter().any(|t| under(t, &["title", "description"], &["field-", "value-"]))
    }

    fn term_allowed(&self, id: &str) -> bool {
        id.starts_with(&format!("{}-", self.plugin))
    }
}

/// Normalizes a locale code (`UK`, `uk_UA` -> `uk`, `uk-ua`); `None` if it is not a plausible code.
pub fn normalize_locale(code: &str) -> Option<String> {
    let code = code.trim().replace('_', "-").to_ascii_lowercase();
    let ok = (2..=16).contains(&code.len())
        && code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && code.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    ok.then_some(code)
}

/// Validates one plugin resource. `Err` when the syntax is invalid (the whole resource is rejected);
/// otherwise the source reduced to the messages the plugin may define.
pub fn filter_plugin_resource(plugin: &str, locale: &str, source: &str, allowed: &AllowedIds) -> Result<String, String> {
    let mut resource = parser::parse(source).map_err(|(_, errors)| {
        format!("{} syntax error(s), first: {:?}", errors.len(), errors.first().map(|e| &e.kind))
    })?;
    let mut dropped = Vec::new();
    resource.body.retain(|entry| match entry {
        Entry::Message(m) => {
            let ok = allowed.message_allowed(m.id.name);
            if !ok {
                dropped.push(m.id.name.to_string());
            }
            ok
        }
        Entry::Term(t) => {
            let ok = allowed.term_allowed(t.id.name);
            if !ok {
                dropped.push(format!("-{}", t.id.name));
            }
            ok
        }
        _ => false,
    });
    if !dropped.is_empty() {
        warn!(plugin, locale, ids = ?dropped, "Translation ids outside the plugin's allowed prefixes were dropped");
    }
    Ok(serializer::serialize(&resource))
}

/// Merges resources per locale in the given order. A message or term already defined earlier
/// (core first, then plugins) wins; later duplicates are dropped silently.
pub fn merge_resources<'a>(sources: impl IntoIterator<Item = (&'a str, &'a str)>) -> Translations {
    let mut seen: BTreeMap<String, HashSet<String>> = BTreeMap::new();
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    for (locale, source) in sources {
        let mut resource: Resource<&str> = match parser::parse(source) {
            Ok(r) => r,
            Err((_, errors)) => {
                warn!(locale, errors = errors.len(), "Skipping a translation resource with syntax errors");
                continue;
            }
        };
        let seen = seen.entry(locale.to_string()).or_default();
        resource.body.retain(|entry| match entry {
            Entry::Message(m) => seen.insert(m.id.name.to_string()),
            Entry::Term(t) => seen.insert(format!("-{}", t.id.name)),
            _ => false,
        });
        let text = merged.entry(locale.to_string()).or_default();
        text.push_str(&serializer::serialize(&resource));
        text.push('\n');
    }
    merged.into_iter().collect()
}

/// Message ids defined in a Fluent source (for tests and diagnostics).
pub fn message_ids(source: &str) -> Vec<String> {
    parser::parse(source)
        .map(|r| r.body)
        .unwrap_or_else(|(r, _)| r.body)
        .into_iter()
        .filter_map(|e| match e {
            Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect()
}

/// A plugin-supplied UI message may only use keys from the plugin's own namespace; any other key is
/// dropped (the English fallback is still shown).
pub fn sanitize_ui_message(mut message: UiMessage, allowed: &AllowedIds) -> UiMessage {
    if !message.key.is_empty() && !message.key.starts_with(&allowed.free_prefix()) {
        warn!(key = %message.key, "Plugin UI error key outside the plugin namespace was dropped");
        message.key.clear();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TickEngine;
    use std::collections::BTreeSet;

    fn bundle(source: &str) -> std::collections::BTreeMap<String, String> {
        let res = parser::parse(source).expect("core translations must parse without errors");
        res.body.into_iter().filter_map(|e| match e {
            Entry::Message(m) => {
                let text: String = m.value.iter().flat_map(|p| p.elements.iter()).map(|el| match el {
                    fluent_syntax::ast::PatternElement::TextElement { value } => *value,
                    fluent_syntax::ast::PatternElement::Placeable { .. } => "{?}",
                }).collect();
                Some((m.id.name.to_string(), text))
            }
            _ => None,
        }).collect()
    }

    fn ids(source: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        for id in message_ids(source) {
            assert!(seen.insert(id.clone()), "duplicate id {id}");
        }
        seen
    }

    #[test]
    fn core_locales_parse_and_have_identical_keys() {
        let [(_, en), (_, uk)] = CORE_TRANSLATIONS;
        assert!(parser::parse(en).is_ok() && parser::parse(uk).is_ok());
        let (en, uk) = (ids(en), ids(uk));
        let only_en: Vec<_> = en.difference(&uk).collect();
        let only_uk: Vec<_> = uk.difference(&en).collect();
        assert!(only_en.is_empty() && only_uk.is_empty(), "only in en: {only_en:?}, only in uk: {only_uk:?}");
    }

    #[test]
    fn every_built_in_node_has_title_description_and_help_in_both_locales() {
        let engine = TickEngine::new();
        let [(_, en), (_, uk)] = CORE_TRANSLATIONS;
        let (en, uk) = (ids(en), ids(uk));
        let definitions = engine.registry.get_definitions();
        assert!(definitions.len() > 20);
        for def in definitions {
            let slug = fluent_slug(&def.type_name);
            for suffix in ["title", "description", "help"] {
                let key = format!("node-{slug}-{suffix}");
                assert!(en.contains(&key), "{key} missing in en.ftl");
                assert!(uk.contains(&key), "{key} missing in uk.ftl");
            }
            assert!(!def.help.trim().is_empty(), "{} has no help", def.type_name);
        }
    }

    /// The definitions are the English fallback: en.ftl must say the same.
    #[test]
    fn english_resource_matches_the_definitions() {
        let engine = TickEngine::new();
        let en = CORE_TRANSLATIONS[0].1;
        let texts = bundle(en);
        for def in engine.registry.get_definitions() {
            let slug = fluent_slug(&def.type_name);
            // The variable nodes put the variable name into their title
            if !def.type_name.starts_with("variable/") {
                assert_eq!(texts[&format!("node-{slug}-title")], def.title, "{}", def.type_name);
            }
            assert_eq!(texts[&format!("node-{slug}-description")], def.description, "{}", def.type_name);
            assert_eq!(texts[&format!("node-{slug}-help")], def.help.trim(), "{}", def.type_name);
            for pin in def.pins.iter().filter(|p| p.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
                if let Some(label) = texts.get(&format!("node-{slug}-pin-{}", pin.name)) {
                    assert_eq!(label, &pin.name, "{} pin {}", def.type_name, pin.name);
                }
            }
        }
    }

    fn allowed() -> AllowedIds {
        AllowedIds::new("my_plugin", &["my/node".to_string()], &["my.type".to_string()])
    }

    #[test]
    fn plugin_resources_are_filtered_by_prefix() {
        let source = "\
node-my-node-title = Title
node-my-node-pin-In = In
node-my-node-prop-entity_id-title = Entity
node-my-node-prop-mode-option-every = Every
node-my-node-prop- = empty
node-other-prop-x-title = stolen
node-my-node-bogus = no
node-other-title = stolen
node-math-add-title = hijack
type-my-type-field-x = X
type-my-type-value-a = A
type-other-title = stolen
setting-my-plugin-key-title = K
setting-other-key-title = stolen
my-plugin-error-x = E { $n }
category-weather = Weather
-my-plugin-term = T
-other-term = T
";
        let out = filter_plugin_resource("my_plugin", "uk", source, &allowed()).unwrap();
        let kept = message_ids(&out);
        assert_eq!(kept, [
            "node-my-node-title", "node-my-node-pin-In", "node-my-node-prop-entity_id-title", "node-my-node-prop-mode-option-every",
            "type-my-type-field-x", "type-my-type-value-a",
            "setting-my-plugin-key-title", "my-plugin-error-x", "category-weather",
        ]);
        assert!(out.contains("-my-plugin-term") && !out.contains("-other-term"));
    }

    #[test]
    fn syntax_errors_reject_the_whole_resource() {
        assert!(filter_plugin_resource("my_plugin", "uk", "node-my-node-title = ok\nthis is = = not fluent {", &allowed()).is_err());
    }

    #[test]
    fn merging_keeps_the_first_definition_and_ignores_broken_sources() {
        let merged = merge_resources([
            ("en", "a = core\nb = core"),
            ("en", "a = plugin\nc = plugin"),
            ("uk", "a = core-uk"),
            ("uk", "= broken {"),
        ]);
        let en = merged.iter().find(|(l, _)| l == "en").unwrap().1.clone();
        assert!(en.contains("a = core") && en.contains("c = plugin") && !en.contains("a = plugin"), "{en}");
        assert!(merged.iter().any(|(l, t)| l == "uk" && t.contains("a = core-uk")));
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn locale_codes_are_normalized() {
        assert_eq!(normalize_locale("UK").as_deref(), Some("uk"));
        assert_eq!(normalize_locale("pt_BR").as_deref(), Some("pt-br"));
        assert_eq!(normalize_locale("../x"), None);
        assert_eq!(normalize_locale(""), None);
    }

    #[test]
    fn ui_error_keys_are_limited_to_the_plugin_namespace() {
        let ok = sanitize_ui_message(UiMessage::new("my-plugin-error-x", "X"), &allowed());
        assert_eq!(ok.key, "my-plugin-error-x");
        let bad = sanitize_ui_message(UiMessage::new("error-cycle-detected", "X"), &allowed());
        assert_eq!((bad.key.as_str(), bad.fallback.as_str()), ("", "X"));
    }

    #[test]
    fn engine_merges_core_translations() {
        let engine = TickEngine::new();
        let translations = engine.translations();
        assert!(translations.iter().any(|(l, t)| l == "en" && t.contains("node-const-title")));
        assert!(translations.iter().any(|(l, t)| l == "uk" && t.contains("node-const-title")));
    }
}

#[cfg(test)]
#[path = "translations/tests.rs"]
mod contract_tests;
