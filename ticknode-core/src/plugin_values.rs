//! Engine-owned read-only values and reference-counted per-node usage.
use std::collections::{BTreeMap, BTreeSet};
use ticknode_shared::*;

#[derive(Default)]
pub struct PluginValues {
    pub(crate) catalogs: crate::value_catalog::ValueCatalogs,
    pub(crate) values: BTreeMap<String, DataValue>,
    usage: BTreeMap<NodeId, BTreeSet<String>>,
    counts: BTreeMap<String, usize>,
    pub(crate) dirty_catalogs: BTreeSet<String>,
}
impl PluginValues {
    pub fn namespaces(&self) -> Vec<String> {
        self.catalogs.keys().cloned().collect()
    }
    pub fn catalogs(&self) -> crate::value_catalog::ValueCatalogs {
        self.catalogs.clone()
    }
    pub fn entry_path(&self, path: &str) -> Option<String> {
        let (namespace, relative) = path.split_once('.')?;
        let (key, _) = self.catalogs.get(namespace)?.entry_path(relative)?;
        Some(format!("{namespace}.{key}"))
    }
    pub fn resolve(&self, path: &str) -> Option<(&DataValue, DataType)> {
        let (namespace, relative) = path.split_once('.')?;
        let catalog = self.catalogs.get(namespace)?;
        let (key, suffix) = catalog.entry_path(relative)?;
        let entry = format!("{namespace}.{key}");
        if !self.counts.contains_key(&entry) {
            return None;
        }
        let value = plugin_values::project(self.values.get(&entry)?, suffix)?;
        let ty = catalog
            .field_type(key, suffix)
            .cloned()
            .unwrap_or_else(|| value.get_type());
        Some((value, ty))
    }
    pub(crate) fn set_usage(&mut self, node: NodeId, paths: BTreeSet<String>) {
        let old = self.usage.remove(&node).unwrap_or_default();
        for path in old.difference(&paths) {
            if let Some(count) = self.counts.get_mut(path) {
                *count -= 1;
                if *count == 0 {
                    self.counts.remove(path);
                }
            }
        }
        for path in paths.difference(&old) {
            *self.counts.entry(path.clone()).or_default() += 1;
        }
        if !paths.is_empty() {
            self.usage.insert(node, paths);
        }
    }
    pub(crate) fn prune_unwatched(&mut self) {
        self.values.retain(|path, _| self.counts.contains_key(path));
    }
    pub(crate) fn is_watched(&self, path: &str) -> bool {
        self.counts.contains_key(path)
    }
    pub(crate) fn watched(&self, namespace: &str) -> BTreeSet<String> {
        let prefix = format!("{namespace}.");
        self.counts
            .keys()
            .filter_map(|p| p.strip_prefix(&prefix).map(str::to_string))
            .collect()
    }
    pub fn snapshot(&self) -> Vec<(String, Option<DataValue>, DataType)> {
        let mut result = Vec::new();
        for path in self.counts.keys() {
            let (namespace, key) = match path.split_once('.') {
                Some(p) => p,
                None => continue,
            };
            let Some(entry) = self
                .catalogs
                .get(namespace)
                .and_then(|catalog| catalog.fields(key))
            else {
                continue;
            };
            result.push((
                path.clone(),
                self.values.get(path).cloned(),
                DataType::Object,
            ));
            for field in entry {
                let full = format!("{path}.{}", field.path);
                result.push((
                    full.clone(),
                    self.resolve(&full).map(|(v, _)| v.clone()),
                    field.data_type.clone(),
                ));
            }
        }
        result
    }
    pub fn for_sheet(
        &self,
        sheet: SheetId,
        node_sheets: &std::collections::HashMap<NodeId, SheetId>,
    ) -> Vec<PluginValue> {
        let paths: BTreeSet<_> = self
            .usage
            .iter()
            .filter(|(node, _)| node_sheets.get(node) == Some(&sheet))
            .flat_map(|(_, paths)| paths.iter())
            .collect();
        paths
            .into_iter()
            .map(|path| PluginValue {
                path: path.clone(),
                value: self.values.get(path).cloned(),
            })
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests;
