//! Incremental editor lookups over slim catalogs and separately fetched field contracts.
use super::*;
use crate::{TypeRegistry, effective_type};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct CatalogBranch {
    pub children: BTreeMap<String, CatalogBranch>,
    pub entry: Option<ValueEntrySummary>,
}

/// Built once per catalog replacement; completion visits only one level and its matching prefix.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct ValueCatalogTree {
    pub roots: BTreeMap<String, CatalogBranch>,
}

impl ValueCatalogTree {
    pub fn replace(&mut self, catalog: &ValueCatalog) {
        let mut root = CatalogBranch::default();
        for entry in &catalog.entries {
            let mut branch = &mut root;
            for part in entry.key.split('.') {
                branch = branch.children.entry(part.into()).or_default();
            }
            branch.entry = Some(entry.clone());
        }
        self.roots.remove(&catalog.namespace);
        if !catalog.entries.is_empty() {
            self.roots.insert(catalog.namespace.clone(), root);
        }
    }

    pub fn branch(&self, path: &str) -> Option<&CatalogBranch> {
        let mut parts = path.split('.');
        let mut branch = self.roots.get(parts.next()?)?;
        for part in parts {
            branch = branch.children.get(part)?;
        }
        Some(branch)
    }

    /// Longest catalog entry prefix, followed by the relative field path.
    pub fn entry<'a>(&'a self, path: &'a str) -> Option<(&'a str, &'a ValueEntrySummary, &'a str)> {
        let (namespace, rest) = path.split_once('.')?;
        let mut branch = self.roots.get(namespace)?;
        let mut offset = 0;
        let mut found = None;
        for part in rest.split('.') {
            let Some(next) = branch.children.get(part) else {
                break;
            };
            branch = next;
            offset += part.len();
            if let Some(entry) = &branch.entry {
                found = Some((namespace, entry, rest[offset..].trim_start_matches('.')));
            }
            offset += 1;
        }
        found
    }
}

pub type ValueFieldsCache = HashMap<(String, String), Vec<ValueField>>;

/// Borrowed view: no catalog or field tree is copied while typing.
pub struct ValueTreeContext<'a> {
    pub catalog: &'a ValueCatalogTree,
    pub fields: &'a ValueFieldsCache,
    pub values: &'a [PluginValue],
}

impl ValueTreeContext<'_> {
    pub fn fields(&self, namespace: &str, key: &str) -> Option<&[ValueField]> {
        self.fields
            .get(&(namespace.into(), key.into()))
            .map(Vec::as_slice)
    }

    pub fn value(&self, namespace: &str, key: &str) -> Option<&DataValue> {
        let path = format!("{namespace}.{key}");
        self.values
            .iter()
            .find(|value| value.path == path)?
            .value
            .as_ref()
    }

    /// Catalog declarations win; hidden runtime types refine Any/Object through the shared resolver.
    pub fn field_type(&self, path: &str, types: &TypeRegistry) -> Option<DataType> {
        let (namespace, entry, suffix) = self.catalog.entry(path)?;
        let value = self.value(namespace, &entry.key);
        let declared = self.fields(namespace, &entry.key).and_then(|fields| {
            fields
                .iter()
                .filter(|f| suffix == f.path || suffix.starts_with(&format!("{}.", f.path)))
                .max_by_key(|f| f.path.len())
        });
        if let Some(field) = declared {
            let tail = suffix.strip_prefix(&field.path)?.trim_start_matches('.');
            let value = value.and_then(|v| project(v, &field.path));
            if let Some(ty) = effective_type(value, &field.data_type, tail, types) {
                return Some(ty);
            }
        }
        if suffix.is_empty()
            || self.fields(namespace, &entry.key).is_some_and(|fields| {
                fields
                    .iter()
                    .any(|f| f.path.starts_with(&format!("{suffix}.")))
            })
        {
            return Some(DataType::Object);
        }
        effective_type(value, &DataType::Object, suffix, types)
    }
}

#[cfg(test)]
mod tests;
