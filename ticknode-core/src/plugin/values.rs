//! Bounded publication mailbox; it survives instances but never accepts unrequested values.
use super::{convert::from_wit_data_type, wit};
use crate::value_catalog::ValueCatalogIndex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use ticknode_shared::*;

#[derive(Default)]
pub(super) struct ValueStore {
    pub namespace: Option<String>,
    pub catalog: Arc<ValueCatalogIndex>,
    pub catalog_dirty: bool,
    pub pending: BTreeMap<String, Option<DataValue>>,
    pub watched: BTreeSet<String>,
}
impl ValueStore {
    pub fn catalog(&mut self, entries: Vec<wit::ValueEntry>) -> Vec<String> {
        if self.namespace.is_none() {
            return Vec::new();
        }
        let mut warnings = Vec::new();
        if entries.len() > MAX_OPTIONS_PER_SOURCE {
            warnings.push("Value catalog exceeds entry limit; truncated".into());
        }
        let mut seen = BTreeSet::new();
        let accepted: Vec<_> = entries
            .into_iter()
            .take(MAX_OPTIONS_PER_SOURCE)
            .filter_map(|e| {
                if !plugin_values::valid_path(&e.key)
                    || e.title.len() > MAX_OPTION_STRING_LEN
                    || e.icon
                        .as_ref()
                        .is_some_and(|s| s.len() > MAX_OPTION_STRING_LEN)
                    || !seen.insert(e.key.clone())
                {
                    warnings.push("Invalid or duplicate value entry dropped".into());
                    return None;
                }
                if e.fields.len() > MAX_OPTION_SOURCES {
                    warnings.push("Value entry exceeds field limit; truncated".into());
                }
                let mut paths = BTreeSet::new();
                let fields = e
                    .fields
                    .into_iter()
                    .take(MAX_OPTION_SOURCES)
                    .filter_map(|f| {
                        if !plugin_values::valid_path(&f.path)
                            || matches!(&f.data_type.base, wit::ScalarType::Named(name) if name.len() > MAX_OPTION_STRING_LEN)
                            || f.title.len() > MAX_OPTION_STRING_LEN
                            || !paths.insert(f.path.clone())
                        {
                            warnings.push("Invalid or duplicate value field dropped".into());
                            return None;
                        }
                        Some(ValueField {
                            path: f.path,
                            title: f.title,
                            data_type: from_wit_data_type(f.data_type),
                        })
                    })
                    .collect();
                Some(ValueEntry {
                    key: e.key,
                    title: e.title,
                    icon: e.icon,
                    fields,
                })
            })
            .collect();
        let accepted = Arc::new(ValueCatalogIndex::new(accepted));
        if self.catalog != accepted {
            self.catalog = accepted;
            self.catalog_dirty = true;
        }
        warnings
    }
    pub fn value(&mut self, key: String, value: Option<Vec<u8>>) -> Result<(), String> {
        if self.namespace.is_none() || !self.watched.contains(&key) {
            return Ok(());
        }
        let value = match value {
            None => None,
            Some(bytes) if bytes.len() <= MAX_VARIABLE_VALUE_SIZE => {
                Some(postcard::from_bytes(&bytes).map_err(|_| "Invalid plugin value encoding")?)
            }
            Some(_) => return Err("Plugin value exceeds 64 KiB limit".into()),
        };
        self.pending.insert(key, value);
        Ok(())
    }
    pub fn invalidate(&mut self) {
        self.pending
            .extend(self.watched.iter().map(|key| (key.clone(), None)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_validation_rejects_numeric_list_paths() {
        let mut store = ValueStore {
            namespace: Some("ha".into()),
            ..Default::default()
        };
        let warnings = store.catalog(vec![wit::ValueEntry {
            key: "sensor.one".into(),
            title: "One".into(),
            icon: None,
            fields: [
                "attributes.items",
                "attributes.items.0",
                "attributes.items.1.state",
            ]
            .map(|path| wit::ValueField {
                path: path.into(),
                title: path.into(),
                data_type: wit::DataType {
                    base: wit::ScalarType::Number,
                    list_depth: 0,
                },
            })
            .into(),
        }]);
        assert_eq!(warnings.len(), 2);
        assert_eq!(store.catalog.fields("sensor.one").unwrap().len(), 1);
    }
}
