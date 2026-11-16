//! Namespace ownership, usage reconciliation and batched watch delivery.
use super::host::PluginState;
use super::{
    bindings, lock,
    runtime::{FUEL_PER_CALL, Runtime},
};
use crate::TickEngine;
use ticknode_shared::*;
use wasmtime::Store;

impl Runtime {
    pub(super) fn restore_value_watches(
        &mut self,
        store: &mut Store<PluginState>,
        plugin: &bindings::Plugin,
    ) -> wasmtime::Result<()> {
        let watches = lock(&self.values).watched.clone();
        let added: Vec<_> = watches.iter().cloned().collect();
        store.set_fuel(FUEL_PER_CALL)?;
        plugin
            .ticknode_plugin_nodes()
            .call_watch_values(&mut *store, &added, &[])?;
        self.sent_watches = watches;
        Ok(())
    }
    pub(super) fn deliver_value_watches(&mut self, full: bool) -> wasmtime::Result<()> {
        let watched = lock(&self.values).watched.clone();
        let added: Vec<_> = watched
            .iter()
            .filter(|key| full || !self.sent_watches.contains(*key))
            .cloned()
            .collect();
        let removed: Vec<_> = self.sent_watches.difference(&watched).cloned().collect();
        if !full && added.is_empty() && removed.is_empty() {
            return Ok(());
        }
        let (store, api) = self.api()?;
        let result = api.call_watch_values(store, &added, &removed);
        self.record_fuel(result.is_err());
        result?;
        self.sent_watches = watched;
        Ok(())
    }
}
impl TickEngine {
    pub(crate) fn validate_values_namespace(&self, manifest: &mut PluginManifest) {
        if let Some(namespace) = &manifest.values_namespace {
            let invalid = !plugin_values::valid_path(namespace)
                || namespace.contains('.')
                || plugin_values::reserved_root(namespace)
                || self
                    .plugins
                    .values()
                    .any(|p| p.manifest.values_namespace.as_ref() == Some(namespace));
            if invalid {
                tracing::warn!(plugin = %manifest.name, namespace, "Ignoring conflicting or invalid plugin values namespace");
                manifest.values_namespace = None;
            }
        }
    }
    pub(crate) fn refresh_value_usage(&mut self, id: NodeId) {
        let mut paths = Vec::new();
        if let Some(node) = self.nodes.get(&id) {
            let properties = node.properties();
            let definition = node.definition();
            let property = if definition.type_name == "text/template" {
                Some("template")
            } else if definition.type_name == "math/expression" {
                Some("expression")
            } else {
                None
            };
            if let Some(property) = property
                && let Some((_, DataValue::String(source))) =
                    properties.iter().find(|(name, _)| name == property)
            {
                if property == "template" {
                    if let Ok(compiled) = template::CompiledTemplate::new(source) {
                        paths.extend(
                            compiled
                                .static_paths()
                                .into_iter()
                                .map(|p| p.strip_prefix("text.").unwrap_or(&p).to_string()),
                        );
                    }
                } else if let Ok(compiled) = formula::compile(source) {
                    paths.extend(
                        compiled
                            .references()
                            .into_iter()
                            .filter(|r| r.lookup == VariableLookup::Plugin)
                            .map(|r| r.name),
                    );
                }
            }
        }
        paths.extend(
            self.input_variables
                .get(&id)
                .into_iter()
                .flatten()
                .filter(|(pin, r)| {
                    r.lookup == VariableLookup::Plugin
                        && self.connections.data_source(id, PinId(**pin)).is_none()
                })
                .map(|(_, r)| r.name.clone()),
        );
        let keys = paths
            .iter()
            .filter_map(|path| self.plugin_values.entry_path(path))
            .collect();
        self.plugin_values.set_usage(id, keys);
    }
    pub(crate) fn refresh_all_value_usage(&mut self) {
        let catalogs = self.plugin_values.catalogs();
        for node in self.nodes.values_mut() {
            node.values_catalogs_changed(&catalogs);
        }
        self.clear_definitions();
        for id in self.node_ids() {
            self.refresh_value_usage(id);
        }
        self.pure_cache.clear();
    }
    /// Drains publications without making guest calls; also runs before synchronous pulls.
    pub(crate) fn sync_plugin_values(&mut self) {
        let mut catalog_changed = false;
        let mut value_changed = false;
        for slot in self.plugins.values().filter(|slot| slot.active) {
            let rt = lock(&slot.runtime);
            let mut store = lock(&rt.values);
            let Some(namespace) = store.namespace.clone() else {
                continue;
            };
            if std::mem::take(&mut store.catalog_dirty) {
                self.plugin_values
                    .catalogs
                    .insert(namespace.clone(), store.catalog.clone());
                self.plugin_values.dirty_catalogs.insert(namespace.clone());
                catalog_changed = true;
            }
            for (key, value) in std::mem::take(&mut store.pending) {
                let path = format!("{namespace}.{key}");
                if !self.plugin_values.is_watched(&path) {
                    continue;
                }
                match value {
                    Some(value) => {
                        if self.plugin_values.values.get(&path) != Some(&value) {
                            self.plugin_values.values.insert(path, value);
                            value_changed = true;
                        }
                    }
                    None => value_changed |= self.plugin_values.values.remove(&path).is_some(),
                }
            }
        }
        if catalog_changed {
            self.refresh_all_value_usage();
        }
        if value_changed {
            self.pure_cache.clear();
        }
    }
    pub(crate) fn prepare_value_watches(&mut self) {
        self.sync_plugin_values();
        self.plugin_values.prune_unwatched();
        for slot in self.plugins.values().filter(|slot| slot.active) {
            let rt = lock(&slot.runtime);
            let mut store = lock(&rt.values);
            if let Some(namespace) = &store.namespace {
                store.watched = self.plugin_values.watched(namespace);
            }
        }
    }
    pub fn value_catalogs(&self) -> Vec<ValueCatalog> {
        self.plugin_values
            .catalogs
            .iter()
            .map(|(namespace, index)| index.slim(namespace.clone()))
            .collect()
    }
    pub fn value_entry_fields(&self, namespace: &str, key: &str) -> Vec<ValueField> {
        self.plugin_values
            .catalogs
            .get(namespace)
            .and_then(|index| index.fields(key))
            .unwrap_or_default()
            .to_vec()
    }
    pub fn take_value_catalog_changes(&mut self) -> Vec<ValueCatalog> {
        self.sync_plugin_values();
        std::mem::take(&mut self.plugin_values.dirty_catalogs)
            .into_iter()
            .map(|namespace| {
                self.plugin_values
                    .catalogs
                    .get(&namespace)
                    .map(|index| index.slim(namespace.clone()))
                    .unwrap_or(ValueCatalog {
                        namespace,
                        entries: Vec::new(),
                    })
            })
            .collect()
    }
    pub(crate) fn remove_values_namespace(&mut self, namespace: &str) {
        self.plugin_values.catalogs.remove(namespace);
        self.plugin_values
            .values
            .retain(|key, _| !key.starts_with(&format!("{namespace}.")));
        self.plugin_values.dirty_catalogs.insert(namespace.into());
        self.refresh_all_value_usage();
    }
}

#[cfg(test)]
mod tests;
