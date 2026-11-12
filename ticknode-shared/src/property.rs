//! Node property schema and runtime property options.
//!
//! Node properties are stored as an untyped `Vec<(String, DataValue)>`; optional definitions describe
//! their controls, validation, order and visibility. Undescribed properties retain their legacy rows.

use crate::{DataValue, PinId};
use serde::{Deserialize, Serialize};

mod validation;
pub use validation::{normalize_property_value, parse_property_duration};

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
    /// Wall-clock text or HH:MM:SS; empty means unset. Signed offsets allow a leading sign.
    Duration { signed: bool },
    /// HH:MM or HH:MM:SS; empty means unset.
    Time,
    /// Comma-separated values, stored in option order.
    MultiSelect(Vec<(String, String)>),
    /// Free text edited in a resizable textarea row.
    Multiline,
    /// Persisted snapshot metadata without an editor row.
    Hidden,
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
    /// An empty property uses this data input instead. Appended for protocol compatibility.
    #[serde(default)]
    pub input: Option<PinId>,
}

impl PropertyDefinition {
    fn new(name: &str, title: &str, kind: PropertyKind) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: String::new(),
            kind,
            input: None,
        }
    }

    pub fn text(name: &str, title: &str) -> Self {
        Self::new(name, title, PropertyKind::Text)
    }

    /// `options` — `(value, label)` pairs.
    pub fn select(name: &str, title: &str, options: &[(&str, &str)]) -> Self {
        let options = options
            .iter()
            .map(|(v, l)| ((*v).into(), (*l).into()))
            .collect();
        Self::new(name, title, PropertyKind::Select(options))
    }

    /// A picker over the options the plugin publishes under `source`.
    pub fn dynamic(name: &str, title: &str, source: &str) -> Self {
        Self::new(
            name,
            title,
            PropertyKind::Dynamic {
                source: source.into(),
                plugin: String::new(),
            },
        )
    }

    pub fn duration(name: &str, title: &str, signed: bool) -> Self {
        Self::new(name, title, PropertyKind::Duration { signed })
    }

    pub fn time(name: &str, title: &str) -> Self {
        Self::new(name, title, PropertyKind::Time)
    }

    pub fn multi_select(name: &str, title: &str, options: &[(&str, &str)]) -> Self {
        Self::new(
            name,
            title,
            PropertyKind::MultiSelect(
                options
                    .iter()
                    .map(|(v, l)| ((*v).into(), (*l).into()))
                    .collect(),
            ),
        )
    }

    pub fn multiline(name: &str, title: &str) -> Self {
        Self::new(name, title, PropertyKind::Multiline)
    }

    pub fn hidden(name: &str) -> Self {
        Self::new(name, "", PropertyKind::Hidden)
    }

    pub fn or_input(mut self, pin: u32) -> Self {
        self.input = Some(PinId(pin));
        self
    }

    pub fn uses_input(&self, properties: &[(String, DataValue)]) -> bool {
        self.input.is_some()
            && properties
                .iter()
                .find(|(name, _)| *name == self.name)
                .is_none_or(|(_, value)| matches!(value, DataValue::String(s) if s.is_empty()))
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
/// Room for integration catalogs, per-domain entity lists and target-domain unions.
pub const MAX_OPTION_SOURCES: usize = 256;
pub const MAX_OPTION_STRING_LEN: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeDefinition;

    #[test]
    fn old_property_schemas_default_to_no_replacement_input() {
        let property: PropertyDefinition =
            serde_json::from_str(r#"{"name":"target","title":"Target"}"#).unwrap();
        assert_eq!(property.input, None);
        assert!(!property.uses_input(&[]));
        let property = property.or_input(1);
        assert!(property.uses_input(&[]));
        assert!(property.uses_input(&[("target".into(), "".into())]));
        assert!(!property.uses_input(&[("target".into(), "light.hall".into())]));
    }

    #[test]
    fn property_schema_roundtrips_through_postcard() {
        let def = NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "x/y".into(),
            title: "X".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            pins: vec![],
            properties: vec![
                PropertyDefinition::text("a", "A"),
                PropertyDefinition::select(
                    "mode",
                    "Mode",
                    &[("every", "Every"), ("latest", "Latest")],
                )
                .described("d"),
                PropertyDefinition::dynamic("entity_id", "Entity", "entities").or_input(1),
                PropertyDefinition::duration("offset", "Offset", true),
                PropertyDefinition::time("at", "At"),
                PropertyDefinition::multi_select("days", "Days", &[("mon", "Monday")]),
                PropertyDefinition::multiline("template", "Template"),
                PropertyDefinition::hidden("snapshot"),
            ],
        };
        let back: NodeDefinition =
            postcard::from_bytes(&postcard::to_allocvec(&def).unwrap()).unwrap();
        assert_eq!(back, def);
        let json: NodeDefinition =
            serde_json::from_str(&serde_json::to_string(&def).unwrap()).unwrap();
        assert_eq!(json, def);

        let set = PropertyOptionSet {
            plugin: "p".into(),
            source: "entities".into(),
            options: vec![PropertyOption {
                value: "light.a".into(),
                label: "A".into(),
                detail: "light.a".into(),
                icon: Some("mdi:lightbulb".into()),
            }],
        };
        let back: PropertyOptionSet =
            postcard::from_bytes(&postcard::to_allocvec(&set).unwrap()).unwrap();
        assert_eq!(back, set);
    }
}
