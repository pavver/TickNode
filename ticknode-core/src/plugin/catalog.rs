use ticknode_shared::*;
use crate::TickEngine;
use super::lock;

impl TickEngine {
    /// Translation resources of the core and of every enabled plugin, merged per locale
    /// (core first, then plugins in name order; the first definition of an id wins).
    pub fn translations(&self) -> Translations {
        let plugin_sources = self.plugins.values().filter(|p| p.active).flat_map(|p| p.translations.iter());
        crate::translations::merge_resources(
            crate::translations::CORE_TRANSLATIONS.iter().copied()
                .chain(plugin_sources.map(|(l, s)| (l.as_str(), s.as_str()))),
        )
    }

    /// Options of dynamic properties published by enabled plugins (for a newly connected editor).
    pub fn property_options(&self) -> Vec<PropertyOptionSet> {
        self.plugins.iter().filter(|(_, p)| p.active)
            .flat_map(|(name, p)| lock(&lock(&p.runtime).options).all(name))
            .collect()
    }

    /// Option sources that changed since the last call (an empty list means the source was removed).
    /// The server calls this after every tick and command and broadcasts the result.
    pub fn take_property_option_changes(&mut self) -> Vec<PropertyOptionSet> {
        self.plugins.iter()
            .flat_map(|(name, p)| lock(&lock(&p.runtime).options).take_changes(name))
            .collect()
    }

    pub fn plugin_names(&self) -> Vec<String> {
        self.plugins.keys().cloned().collect()
    }

    pub fn plugin_manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name).map(|p| &p.manifest)
    }

    pub fn plugin_active(&self, name: &str) -> bool {
        self.plugins.get(name).is_some_and(|p| p.active)
    }
}
