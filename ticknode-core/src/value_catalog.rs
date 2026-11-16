//! Immutable namespace indexes shared by the mailbox, engine and nodes.
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use ticknode_shared::{DataType, ValueCatalog, ValueEntry, ValueEntrySummary, ValueField};

pub type ValueCatalogs = BTreeMap<String, Arc<ValueCatalogIndex>>;

#[derive(Debug, Default, PartialEq)]
pub struct ValueCatalogIndex {
    entries: HashMap<String, IndexedEntry>,
    order: Vec<String>,
}
#[derive(Debug, PartialEq)]
struct IndexedEntry {
    entry: ValueEntry,
    types: HashMap<String, DataType>,
}
impl ValueCatalogIndex {
    pub fn new(entries: Vec<ValueEntry>) -> Self {
        let order = entries.iter().map(|entry| entry.key.clone()).collect();
        let entries = entries
            .into_iter()
            .map(|entry| {
                let types = entry
                    .fields
                    .iter()
                    .map(|field| (field.path.clone(), field.data_type.clone()))
                    .collect();
                (entry.key.clone(), IndexedEntry { entry, types })
            })
            .collect();
        Self { entries, order }
    }
    /// Longest dotted entry prefix wins; work depends on path depth, not catalog size.
    pub fn entry_path<'a>(&self, relative: &'a str) -> Option<(&'a str, &'a str)> {
        let mut key = relative;
        loop {
            if self.entries.contains_key(key) {
                let suffix = relative.get(key.len()..)?.strip_prefix('.').unwrap_or("");
                return Some((key, suffix));
            }
            key = key.rsplit_once('.')?.0;
        }
    }
    pub fn field_type(&self, key: &str, path: &str) -> Option<&DataType> {
        self.entries.get(key)?.types.get(path)
    }
    pub fn fields(&self, key: &str) -> Option<&[ValueField]> {
        Some(&self.entries.get(key)?.entry.fields)
    }
    pub fn slim(&self, namespace: String) -> ValueCatalog {
        ValueCatalog {
            namespace,
            entries: self
                .order
                .iter()
                .filter_map(|key| {
                    let entry = &self.entries.get(key)?.entry;
                    Some(ValueEntrySummary {
                        key: entry.key.clone(),
                        title: entry.title.clone(),
                        icon: entry.icon.clone(),
                    })
                })
                .collect(),
        }
    }
}

pub fn catalog_type<'a>(catalogs: &'a ValueCatalogs, path: &str) -> Option<&'a DataType> {
    let (namespace, relative) = path.split_once('.')?;
    let catalog = catalogs.get(namespace)?;
    let (key, suffix) = catalog.entry_path(relative)?;
    catalog.field_type(key, suffix)
}
