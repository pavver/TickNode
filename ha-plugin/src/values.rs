//! Typed namespace values, independent of the legacy ha.state structure.
mod attributes;
use ha_core_client::State;
use std::collections::BTreeMap;
use ticknode_shared::{DataType, DataValue, ObjectField, ValueEntry, ValueField};

type Contracts = BTreeMap<String, DataType>;
pub(super) fn object(fields: impl IntoIterator<Item = (String, DataValue)>) -> DataValue {
    DataValue::Object {
        type_name: None,
        fields: fields
            .into_iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    ObjectField {
                        data_type: Some(value.get_type()),
                        value,
                    },
                )
            })
            .collect(),
    }
}

/// State deserialization already calls AttributeSet::for_entity in ha-core-client.
/// Its domain-specific booleans and timestamps therefore arrive normalized here.
pub fn tree(state: &State, temperature_unit: Option<&str>) -> (DataValue, Vec<ValueField>) {
    let mut contracts = Contracts::new();
    let kind = crate::entity::infer_value_type(state);
    contracts.insert("state".into(), kind.data_type());
    let attrs = attributes::convert(
        &crate::convert::attributes_json(state),
        temperature_unit,
        &mut contracts,
        "attributes",
        0,
    );
    let mut fields = vec![
        ("entity_id".into(), state.entity_id.clone().into()),
        (
            "name".into(),
            state
                .attributes
                .friendly_name()
                .unwrap_or(&state.entity_id)
                .into(),
        ),
        ("state_text".into(), state.state.clone().into()),
        ("attributes".into(), attrs.unwrap_or_else(|| object([]))),
        (
            "last_changed".into(),
            DataValue::DateTime(state.last_changed),
        ),
        (
            "last_updated".into(),
            DataValue::DateTime(state.last_updated),
        ),
    ];
    if let Some(value) = crate::entity::typed_value(state, kind) {
        fields.push(("state".into(), value));
    }
    let mut value = object(fields);
    stamp(&mut value, "", &contracts);
    collect_fields(&value, "", &mut contracts, 0);
    let fields = contracts
        .into_iter()
        .map(|(path, data_type)| ValueField {
            title: path.rsplit('.').next().unwrap_or(&path).replace('_', " "),
            path,
            data_type,
        })
        .collect();
    (value, fields)
}
pub(super) fn stamp(value: &mut DataValue, prefix: &str, contracts: &Contracts) {
    if let DataValue::Object { fields, .. } = value {
        for (key, field) in fields {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            if let Some(ty) = contracts.get(&path) {
                field.data_type = Some(ty.clone());
            }
            stamp(&mut field.value, &path, contracts);
        }
    }
}
fn collect_fields(value: &DataValue, prefix: &str, contracts: &mut Contracts, depth: usize) {
    if depth > 16 {
        return;
    }
    if !prefix.is_empty() {
        contracts
            .entry(prefix.into())
            .or_insert_with(|| value.get_type());
    }
    if let DataValue::Object { fields, .. } = value {
        for (key, field) in fields {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            collect_fields(&field.value, &path, contracts, depth + 1);
        }
    }
}
pub fn catalog(entry: &crate::entities::CatalogEntry) -> ValueEntry {
    ValueEntry {
        key: entry.entity_id.clone(),
        title: entry
            .attributes
            .friendly_name()
            .unwrap_or(&entry.entity_id)
            .into(),
        icon: Some(entry.icon.clone()),
        fields: entry.value_fields.to_vec(),
    }
}

#[cfg(test)]
mod tests;
