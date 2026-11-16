use super::*;
use crate::value_catalog::ValueCatalogIndex;
use std::sync::Arc;

pub(crate) fn large_catalog() -> Arc<ValueCatalogIndex> {
    Arc::new(ValueCatalogIndex::new(
        (0..20_000)
            .map(|i| ValueEntry {
                key: format!("sensor.entity_{i}"),
                title: format!("Entity {i}"),
                icon: None,
                fields: (0..30)
                    .map(|j| ValueField {
                        path: format!("attributes.field_{j}"),
                        title: format!("Field {j}"),
                        data_type: DataType::Named("quantity.power".into()),
                    })
                    .collect(),
            })
            .collect(),
    ))
}
#[test]
fn large_catalog_handles_and_indexed_projection() {
    let catalog = large_catalog();
    let mut values = PluginValues::default();
    values.catalogs.insert("ha".into(), catalog.clone());
    let handles = values.catalogs();
    assert!(Arc::ptr_eq(&catalog, &handles["ha"]));
    assert_eq!(
        catalog.entry_path("sensor.entity_19999.attributes.field_29"),
        Some(("sensor.entity_19999", "attributes.field_29"))
    );
    assert_eq!(
        catalog.field_type("sensor.entity_19999", "attributes.field_29"),
        Some(&DataType::Named("quantity.power".into()))
    );
    assert!(catalog.entry_path("sensor.unknown.state").is_none());
    let path = "ha.sensor.entity_19999";
    values.set_usage(NodeId::generate(), [path.into()].into());
    values.values.insert(
        path.into(),
        DataValue::Object {
            type_name: None,
            fields: [(
                "state".into(),
                ObjectField {
                    name: "state".into(),
                    description: String::new(),
                    value: true.into(),
                },
            )]
            .into(),
        },
    );
    assert_eq!(
        values.resolve(&format!("{path}.state")),
        Some((&DataValue::Bool(true), DataType::Bool))
    );
}
#[test]
fn dotted_keys_use_longest_prefix() {
    let catalog = ValueCatalogIndex::new(
        ["sensor.one", "sensor.one.nested"]
            .into_iter()
            .map(|key| ValueEntry {
                key: key.into(),
                title: key.into(),
                icon: None,
                fields: vec![],
            })
            .collect(),
    );
    assert_eq!(
        catalog.entry_path("sensor.one.nested.state"),
        Some(("sensor.one.nested", "state"))
    );
    assert_eq!(
        catalog.entry_path("sensor.one.nested"),
        Some(("sensor.one.nested", ""))
    );
    assert!(catalog.entry_path("sensor.one_more.state").is_none());
}
