use super::*;

#[test]
fn field_hints_and_nested_inspector_labels_follow_metadata_in_both_locales() {
    let mut definitions = quantity::builtin_quantities();
    definitions.push(TypeDefinition::new_struct(
        "test.meter",
        "Meter",
        vec![
            FieldDefinition::new("power", "Power", DataType::Named("quantity.power".into()))
                .described("Meter reading"),
        ],
    ));
    let types = TypeRegistry::from_definitions(definitions);
    let tree = DataValue::Object {
        type_name: None,
        fields: [(
            "power".into(),
            ObjectField {
                value: 1200i64.into(),
                data_type: Some(DataType::Named("quantity.power".into())),
            },
        )]
        .into(),
    };
    let resources = vec![
        (
            "en".into(),
            format!(
                "{}\ntype-test-meter-field-power-description = Meter reading\n",
                include_str!("../../../ticknode-core/locales/en.ftl")
            ),
        ),
        (
            "uk".into(),
            format!(
                "{}\ntype-test-meter-field-power-description = Показ лічильника\n",
                include_str!("../../../ticknode-core/locales/uk.ftl")
            ),
        ),
    ];
    set_runtime_translations(&resources);
    set_known_types(&types);
    for (locale, reading, quantity, shown) in [
        (
            Locale::En,
            "Meter reading",
            "Power in physical units",
            "1.2 kW",
        ),
        (
            Locale::Uk,
            "Показ лічильника",
            "Потужність: фізична величина з одиницями вимірювання",
            "1,2 кВт",
        ),
    ] {
        with_locale(locale, || {
            let hint = value_tree::field_hint(
                Some(&tree),
                &DataType::Named("test.meter".into()),
                "power",
                &types,
            );
            assert_eq!(ui_text(&hint), reading);
            let hint = value_tree::field_hint(Some(&tree), &DataType::Object, "power", &types);
            assert_eq!(ui_text(&hint), quantity);
            assert_eq!(
                crate::panels::display_value_in(&tree, &DataType::Object),
                format!("{{power: {shown}}}")
            );
            assert_eq!(
                value_as_text(&tree, &DataType::Any),
                format!("{{power: {shown}}}")
            );
        });
    }
    set_known_types(&TypeRegistry::new());
    set_runtime_translations(&[]);
}
