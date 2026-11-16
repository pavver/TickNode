//! Read-only plugin catalogs and synchronous typed path projection.
use crate::{DataType, DataValue};
use serde::{Deserialize, Serialize};

pub mod editor;
pub mod lint;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValueField {
    pub path: String,
    pub title: String,
    pub data_type: DataType,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueEntry {
    pub key: String,
    pub title: String,
    pub icon: Option<String>,
    pub fields: Vec<ValueField>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueEntrySummary {
    pub key: String,
    pub title: String,
    pub icon: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueCatalog {
    pub namespace: String,
    pub entries: Vec<ValueEntrySummary>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginValue {
    /// Full path including the namespace and entry key.
    pub path: String,
    pub value: Option<DataValue>,
}

pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= crate::MAX_OPTION_STRING_LEN
        && path.split('.').all(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

pub fn reserved_root(root: &str) -> bool {
    matches!(
        root,
        "var"
            | "sheet"
            | "global"
            | "text"
            | "loop"
            | "self"
            | "super"
            | "true"
            | "false"
            | "none"
            | "True"
            | "False"
            | "None"
            | "caller"
    ) || crate::template::environment()
        .globals()
        .any(|(name, _)| name == root)
}

pub fn project<'a>(mut value: &'a DataValue, path: &str) -> Option<&'a DataValue> {
    if path.is_empty() {
        return Some(value);
    }
    for field in path.split('.') {
        value = match value {
            DataValue::Object { fields, .. } => &fields.get(field)?.value,
            DataValue::List { elements, .. } => elements.get(field.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}
