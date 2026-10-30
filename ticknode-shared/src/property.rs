//! Node property schema and runtime property options.
//!
//! Node properties are stored as an untyped `Vec<(String, DataValue)>`; a node may additionally describe some
//! of them (`NodeDefinition::properties`) so the editor can show a title, a drop-down or a searchable picker
//! instead of a plain text field. A property that is not described keeps the default text row.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PropertyKind {
    /// Free text (the default behaviour for a string property).
    #[default]
    Text,
    /// A drop-down with static `(value, label)` options.
    Select(Vec<(String, String)>),
    /// A searchable picker whose options are published at runtime by the node's plugin under `source`
    /// (see `PropertyOption`). `plugin` is filled by the host with the owning plugin's name; plugins leave it empty.
    Dynamic {
        source: String,
        #[serde(default)]
        plugin: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyDefinition {
    /// The property name as used by `set_property` / `properties()`.
    pub name: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub kind: PropertyKind,
}

impl PropertyDefinition {
    fn new(name: &str, title: &str, kind: PropertyKind) -> Self {
        Self { name: name.into(), title: title.into(), description: String::new(), kind }
    }

    pub fn text(name: &str, title: &str) -> Self {
        Self::new(name, title, PropertyKind::Text)
    }

    /// `options` — `(value, label)` pairs.
    pub fn select(name: &str, title: &str, options: &[(&str, &str)]) -> Self {
        let options = options.iter().map(|(v, l)| ((*v).into(), (*l).into())).collect();
        Self::new(name, title, PropertyKind::Select(options))
    }

    /// A picker over the options the plugin publishes under `source`.
    pub fn dynamic(name: &str, title: &str, source: &str) -> Self {
        Self::new(name, title, PropertyKind::Dynamic { source: source.into(), plugin: String::new() })
    }

    pub fn described(mut self, description: &str) -> Self {
        self.description = description.into();
        self
    }
}

/// One option of a dynamic property, published by a plugin (`host::set-options`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyOption {
    /// The value stored in the property.
    pub value: String,
    /// Primary line.
    pub label: String,
    /// Secondary line, e.g. the entity id.
    pub detail: String,
    /// Material Design Icons name, e.g. `mdi:lightbulb`.
    pub icon: Option<String>,
}

/// All options a plugin published under one source (sent to the editor).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PropertyOptionSet {
    pub plugin: String,
    pub source: String,
    pub options: Vec<PropertyOption>,
}

/// Limits applied by the host to `set-options`.
pub const MAX_OPTIONS_PER_SOURCE: usize = 20_000;
pub const MAX_OPTION_SOURCES: usize = 16;
pub const MAX_OPTION_STRING_LEN: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeDefinition;

    #[test]
    fn property_schema_roundtrips_through_postcard() {
        let def = NodeDefinition {
            type_name: "x/y".into(), title: "X".into(), category: "Test".into(), description: String::new(), help: String::new(),
            pins: vec![],
            properties: vec![
                PropertyDefinition::text("a", "A"),
                PropertyDefinition::select("mode", "Mode", &[("every", "Every"), ("latest", "Latest")]).described("d"),
                PropertyDefinition::dynamic("entity_id", "Entity", "entities"),
            ],
        };
        let back: NodeDefinition = postcard::from_bytes(&postcard::to_allocvec(&def).unwrap()).unwrap();
        assert_eq!(back, def);
        let json: NodeDefinition = serde_json::from_str(&serde_json::to_string(&def).unwrap()).unwrap();
        assert_eq!(json, def);

        let set = PropertyOptionSet {
            plugin: "p".into(), source: "entities".into(),
            options: vec![PropertyOption { value: "light.a".into(), label: "A".into(), detail: "light.a".into(), icon: Some("mdi:lightbulb".into()) }],
        };
        let back: PropertyOptionSet = postcard::from_bytes(&postcard::to_allocvec(&set).unwrap()).unwrap();
        assert_eq!(back, set);
    }
}
