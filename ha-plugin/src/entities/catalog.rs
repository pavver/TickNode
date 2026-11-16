//! Picker metadata retained after the one-shot state catalog has been discarded.
use ha_core_client::State;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub entity_id: String,
    pub attributes: MetadataAttributes,
    pub icon: String,
    pub value_fields: std::sync::Arc<[ticknode_shared::ValueField]>,
    pub value_type: crate::entity::ValueType,
}
#[derive(Clone, Debug)]
pub struct MetadataAttributes(BTreeMap<String, Value>);
impl MetadataAttributes {
    pub fn get_as<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        serde_json::from_value(self.0.get(key)?.clone()).ok()
    }
    pub fn friendly_name(&self) -> Option<&str> {
        self.0.get("friendly_name")?.as_str()
    }
}
pub trait EntityMetadata {
    fn metadata(&self) -> Cow<'_, CatalogEntry>;
}
impl EntityMetadata for CatalogEntry {
    fn metadata(&self) -> Cow<'_, CatalogEntry> {
        Cow::Borrowed(self)
    }
}
impl EntityMetadata for State {
    fn metadata(&self) -> Cow<'_, CatalogEntry> {
        Cow::Owned(CatalogEntry::from(self))
    }
}
impl From<&State> for CatalogEntry {
    fn from(state: &State) -> Self {
        Self::with_fields(state, crate::values::tree(state, None).1.into())
    }
}
impl CatalogEntry {
    pub fn with_fields(
        state: &State,
        value_fields: std::sync::Arc<[ticknode_shared::ValueField]>,
    ) -> Self {
        let attributes = [
            "friendly_name",
            "device_class",
            "entity_category",
            "unit_of_measurement",
            "temperature_unit",
            "supported_features",
        ]
        .into_iter()
        .filter_map(|key| state.attributes.get(key).map(|v| (key.into(), v)))
        .collect();
        let class: Option<String> = state.attributes.get_as("device_class");
        Self {
            entity_id: state.entity_id.clone(),
            attributes: MetadataAttributes(attributes),
            icon: crate::icons::entity_icon(
                &state.entity_id,
                &state.state,
                class.as_deref(),
                state.attributes.icon(),
            ),
            value_type: crate::entity::infer_value_type(state),
            value_fields,
        }
    }
}
