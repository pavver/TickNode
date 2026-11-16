use super::*;
use crate::template::{Completion, CompletionInput};
use crate::{ObjectField, ValueCatalog, ValueEntrySummary, ValueField, formula, template};

fn catalog() -> ValueCatalogTree {
    let mut tree = ValueCatalogTree::default();
    tree.replace(&ValueCatalog {
        namespace: "ha".into(),
        entries: vec![
            ValueEntrySummary {
                key: "sensor.kitchen".into(),
                title: "Kitchen temperature".into(),
                icon: Some("mdi:thermometer".into()),
            },
            ValueEntrySummary {
                key: "light.kitchen".into(),
                title: "Kitchen light".into(),
                icon: None,
            },
        ],
    });
    tree
}

fn fields() -> ValueFieldsCache {
    [(
        ("ha".into(), "sensor.kitchen".into()),
        [
            ("state", DataType::Named("quantity.temperature".into())),
            ("state_text", DataType::String),
            ("last_changed", DataType::DateTime),
            ("attributes", DataType::Object),
            (
                "attributes.room.temperature",
                DataType::Named("quantity.temperature".into()),
            ),
            ("attributes.room.enabled", DataType::Bool),
            ("attributes.color", DataType::Color),
            ("attributes.duration", DataType::Duration),
            (
                "attributes.readings",
                DataType::List(Box::new(DataType::Named("quantity.power".into()))),
            ),
        ]
        .into_iter()
        .map(|(path, data_type)| ValueField {
            path: path.into(),
            title: path.into(),
            data_type,
        })
        .collect(),
    )]
    .into()
}

fn complete(path: &str, ctx: &ValueTreeContext, formula: bool) -> Completion {
    let types = TypeRegistry::from_definitions(crate::quantity::catalog().iter().cloned());
    let source = if formula {
        path.into()
    } else {
        format!("{{{{ {path}")
    };
    let inputs = [("value".into(), DataType::Number)];
    let input = CompletionInput {
        source: &source,
        caret: source.len(),
        inputs: &inputs,
        variables: &[],
        types: &types,
        now: None,
        labels: None,
    };
    if formula {
        formula::complete::complete_with_catalog(&input, ctx)
    } else {
        template::complete::complete_with_catalog(&input, &[], Some(ctx)).unwrap()
    }
}

#[test]
fn completion_walks_slim_catalog_by_level_and_retains_entity_metadata() {
    let catalog = catalog();
    let fields = fields();
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &[],
    };
    for formula in [false, true] {
        let roots = complete("h", &ctx, formula);
        assert!(roots.items.iter().any(|i| i.label == "ha"));
        let domains = complete("ha.", &ctx, formula);
        assert_eq!(
            domains
                .items
                .iter()
                .map(|i| i.label.as_str())
                .collect::<Vec<_>>(),
            ["light", "sensor"]
        );
        let entities = complete("ha.sensor.ki", &ctx, formula);
        assert_eq!(entities.items.len(), 1);
        assert_eq!(entities.items[0].label, "kitchen");
        assert_eq!(entities.items[0].detail, "Kitchen temperature");
        assert_eq!(entities.items[0].icon.as_deref(), Some("mdi:thermometer"));
        assert_eq!(entities.items[0].insert, "kitchen.");
    }
    let roots = complete("text.", &ctx, false);
    assert!(roots.items.iter().any(|i| i.label == "ha"));
    assert!(roots.items.iter().any(|i| i.label == "value"));
}

#[test]
fn cached_nested_contracts_complete_without_any_live_values() {
    let catalog = catalog();
    let fields = fields();
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &[],
    };
    for formula in [false, true] {
        let state = complete("ha.sensor.kitchen.st", &ctx, formula);
        assert_eq!(state.items.len(), 2);
        assert!(
            state.items[0].detail.contains("Temperature"),
            "{:?}",
            state.items
        );
        assert!(state.items[0].detail.contains("°C"));
        assert_eq!(state.items[1].detail, "Text");
        let room = complete("ha.sensor.kitchen.attributes.room.", &ctx, formula);
        assert_eq!(room.items.len(), 2);
        assert_eq!(room.items[0].detail, "Bool");
        assert!(room.items[1].detail.contains("Temperature"));
        let attributes = complete("ha.sensor.kitchen.attributes.", &ctx, formula);
        assert_eq!(
            attributes
                .items
                .iter()
                .map(|i| i.detail.as_str())
                .collect::<Vec<_>>(),
            ["Color", "Duration", "List: Power (W)", "Object"]
        );
    }
    let text = complete("text.ha.sensor.kitchen.last_", &ctx, false);
    assert_eq!(text.items[0].detail, "DateTime");
}

#[test]
fn runtime_field_metadata_refines_objects_but_catalog_contracts_win() {
    let catalog = catalog();
    let fields = fields();
    let value = DataValue::Object {
        type_name: None,
        fields: [
            (
                "state".into(),
                ObjectField {
                    value: 1i64.into(),
                    data_type: Some(DataType::String),
                },
            ),
            (
                "attributes".into(),
                ObjectField {
                    value: DataValue::Object {
                        type_name: None,
                        fields: [(
                            "new_power".into(),
                            ObjectField {
                                value: 10i64.into(),
                                data_type: Some(DataType::Named("quantity.power".into())),
                            },
                        )]
                        .into(),
                    },
                    data_type: None,
                },
            ),
        ]
        .into(),
    };
    let values = vec![PluginValue {
        path: "ha.sensor.kitchen".into(),
        value: Some(value),
    }];
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &values,
    };
    let types = TypeRegistry::from_definitions(crate::quantity::catalog().iter().cloned());
    assert_eq!(
        ctx.field_type("ha.sensor.kitchen.state", &types),
        Some(DataType::Named("quantity.temperature".into()))
    );
    assert_eq!(
        ctx.field_type("ha.sensor.kitchen.attributes.new_power", &types),
        Some(DataType::Named("quantity.power".into()))
    );
    let items = complete("ha.sensor.kitchen.attributes.new_", &ctx, false);
    assert_eq!(items.items[0].detail, "Power (W)");
}

#[test]
fn namespace_lints_distinguish_known_unknown_and_pending_fields() {
    let catalog = catalog();
    let fields = fields();
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &[],
    };
    let types = TypeRegistry::from_definitions(crate::quantity::catalog().iter().cloned());
    for path in [
        "ha.sensor.kitchen.state",
        "ha.sensor.kitchen.attributes.room.temperature",
        "ha.sensor.kitchen.attributes.readings.0",
    ] {
        assert!(
            template::lint::lint_with_catalog(
                &format!("{{{{ text.{path} }}}}"),
                &[],
                &types,
                &[],
                &ctx
            )
            .is_empty(),
            "{path}"
        );
        assert!(
            formula::lint::lint_with_catalog(&format!("{path} * 2"), &[], &types, &ctx).is_empty(),
            "{path}"
        );
    }
    for path in [
        "ha.sensor.missing.state",
        "ha.sensor.kitchen.attributes.room.nope",
    ] {
        let source = format!("{{{{ text.{path} }}}}");
        let lints = template::lint::lint_with_catalog(&source, &[], &types, &[], &ctx);
        assert_eq!(lints.len(), 1, "{path}");
        assert!(!lints[0].message.key.is_empty());
        assert_eq!(
            formula::lint::lint_with_catalog(path, &[], &types, &ctx).len(),
            1
        );
    }
    let pending = ValueFieldsCache::new();
    let ctx = ValueTreeContext {
        fields: &pending,
        ..ctx
    };
    assert!(
        template::lint::lint_with_catalog(
            "{{ ha.sensor.kitchen.unknown }}",
            &[],
            &types,
            &[],
            &ctx
        )
        .is_empty()
    );
    assert!(
        template::lint::lint_with_catalog(
            "ha.sensor.missing {{ 'ha.sensor.missing' }} {# ha.sensor.missing #}",
            &[],
            &types,
            &[],
            &ctx
        )
        .is_empty()
    );
}

#[test]
fn twenty_thousand_entries_are_filtered_by_level_and_prefix_with_bounded_results() {
    let mut catalog = ValueCatalogTree::default();
    catalog.replace(&ValueCatalog {
        namespace: "ha".into(),
        entries: (0..20_000)
            .map(|i| ValueEntrySummary {
                key: format!("sensor.entity_{i:05}"),
                title: format!("Entity {i}"),
                icon: None,
            })
            .collect(),
    });
    let fields = ValueFieldsCache::new();
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &[],
    };
    let domains = complete("ha.", &ctx, false);
    assert_eq!(domains.items.len(), 1);
    let all = complete("ha.sensor.", &ctx, false);
    assert_eq!(all.items.len(), 50);
    let narrowed = complete("ha.sensor.entity_1999", &ctx, false);
    assert_eq!(narrowed.items.len(), 10);
    assert_eq!(narrowed.items[9].label, "entity_19999");
    assert!(ctx.fields.is_empty());
}

#[test]
fn named_structs_inside_lists_complete_and_lint_through_the_same_contract() {
    let catalog = catalog();
    let mut fields = fields();
    fields
        .get_mut(&("ha".into(), "sensor.kitchen".into()))
        .unwrap()
        .push(ValueField {
            path: "attributes.items".into(),
            title: "Readings".into(),
            data_type: DataType::List(Box::new(DataType::Named("demo.reading".into()))),
        });
    let types = TypeRegistry::from_definitions([crate::TypeDefinition::new_struct(
        "demo.reading",
        "Reading",
        vec![
            crate::FieldDefinition::new("color", "Color", DataType::Color),
            crate::FieldDefinition::new("duration", "Duration", DataType::Duration),
        ],
    )]);
    let ctx = ValueTreeContext {
        catalog: &catalog,
        fields: &fields,
        values: &[],
    };
    let source = "{{ ha.sensor.kitchen.attributes.items.0.";
    let input = CompletionInput {
        source,
        caret: source.len(),
        inputs: &[],
        variables: &[],
        types: &types,
        now: None,
        labels: None,
    };
    let completed = template::complete::complete_with_catalog(&input, &[], Some(&ctx)).unwrap();
    assert_eq!(
        completed
            .items
            .iter()
            .map(|i| i.detail.as_str())
            .collect::<Vec<_>>(),
        ["Color", "Duration"]
    );
    assert_eq!(
        ctx.field_type("ha.sensor.kitchen.attributes.items.0.duration", &types),
        Some(DataType::Duration)
    );
    assert!(
        template::lint::lint_with_catalog(
            "{{ ha.sensor.kitchen.attributes.items.0.duration }}",
            &[],
            &types,
            &[],
            &ctx
        )
        .is_empty()
    );
    assert_eq!(
        template::lint::lint_with_catalog(
            "{{ ha.sensor.kitchen.attributes.items.0.missing }}",
            &[],
            &types,
            &[],
            &ctx
        )
        .len(),
        1
    );
    assert_eq!(
        formula::lint::lint_with_catalog(
            "ha.sensor.kitchen.attributes.items.0.missing",
            &[],
            &types,
            &ctx
        )
        .len(),
        1
    );
    assert!(
        template::lint::lint_with_catalog(
            "{% for ha in values %}{{ ha.unknown }}{% endfor %}",
            &[],
            &types,
            &[],
            &ctx
        )
        .is_empty()
    );
}
