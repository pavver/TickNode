use ticknode_shared::*;

#[test]
fn registry_titles_resolve_nested_lists_and_missing_types() {
    let registry =
        TypeRegistry::from_definitions([TypeDefinition::new_struct("t.point", "Point", vec![])]);
    assert_eq!(
        registry.title_of(&DataType::Named("t.point".into())),
        "Point"
    );
    assert_eq!(
        registry.title_of(&DataType::Named("t.missing".into())),
        "t.missing"
    );
    assert_eq!(
        registry.title_of(&DataType::List(Box::new(DataType::Named("t.point".into())))),
        "list<Point>"
    );
    assert_eq!(registry.title_of(&DataType::Bool), "bool");
}

#[test]
fn setting_builders_preserve_optional_and_required_schema_flags() {
    for setting in [
        SettingDefinition::text("label", "Label"),
        SettingDefinition::number("limit", "Limit"),
        SettingDefinition::bool("enabled", "Enabled"),
    ] {
        assert!(!setting.required);
        assert_eq!(setting.default, None);
        let required = setting.clone().required();
        assert_eq!(
            required,
            SettingDefinition {
                required: true,
                ..setting
            }
        );
    }
}

#[test]
fn text_and_numeric_settings_accept_their_values_and_reject_mismatches() {
    for (setting, value) in [
        (
            SettingDefinition::text("name", "Name"),
            DataValue::from("Kitchen"),
        ),
        (
            SettingDefinition::secret("token", "Token"),
            DataValue::from("secret"),
        ),
        (
            SettingDefinition::number("limit", "Limit"),
            DataValue::from(12i64),
        ),
        (
            SettingDefinition::number("ratio", "Ratio"),
            DataValue::from(0.5),
        ),
    ] {
        assert_eq!(setting.check("sample", &value), Ok(()));
        let error = setting
            .check("sample", &DataValue::Bool(false))
            .unwrap_err();
        assert_eq!(error.key, "error-setting-invalid");
        assert!(error.args.contains(&("key".into(), setting.key.clone())));
        assert!(error.args.contains(&("plugin".into(), "sample".into())));
    }
}

#[test]
fn field_builders_preserve_required_and_optional_schema_flags() {
    let field = FieldDefinition::new("x", "X", DataType::Number);
    assert!(!field.optional);
    assert_eq!(
        field.clone().optional(),
        FieldDefinition {
            optional: true,
            ..field.clone()
        }
    );
    let definition =
        TypeDefinition::new_struct("t.point", "Point", vec![field.clone(), field.optional()]);
    assert!(!definition.fields()[0].optional);
    assert!(definition.fields()[1].optional);
}

#[test]
fn custom_type_names_accept_digits_underscores_and_hyphens_but_reject_invalid_segments() {
    for name in ["t1.field_2", "t1.field-2", "1.2", "_.-", "plugin.record"] {
        let mut types = TypeRegistry::new();
        let definition = TypeDefinition::new_enum(name, "Mode", &[("a", "A")]);
        types.insert_all(std::slice::from_ref(&definition)).unwrap();
        assert_eq!(types.get(name), Some(&definition));
        assert_eq!(parse_data_type(name), Some(DataType::Named(name.into())));
    }
    for name in [
        ".record",
        "plugin.",
        "plugin..record",
        "plugin.Record",
        "plugin.rec?ord",
        "plugin.rec ord",
        "plugin.é",
    ] {
        let mut types = TypeRegistry::new();
        assert_eq!(
            types.insert_all(&[TypeDefinition::new_enum(name, "Mode", &[("a", "A")])]),
            Err(format!(
                "Invalid type name {name:?}: expected something like \"plugin.type\""
            ))
        );
        assert!(types.is_empty());
    }
}

#[test]
fn empty_and_duplicate_struct_fields_fail_registration_atomically() {
    for names in [vec![""], vec!["x", "x"]] {
        let mut types = TypeRegistry::new();
        let before = types.definitions();
        let fields = names
            .iter()
            .map(|name| FieldDefinition::new(name, "Field", DataType::Number))
            .collect();
        let result = types.insert_all(&[TypeDefinition::new_struct("t.bad", "Bad", fields)]);
        assert_eq!(
            result,
            Err(format!(
                "Type t.bad: empty or duplicate field name {:?}",
                names.last().unwrap()
            ))
        );
        assert_eq!(types.definitions(), before);
    }
}
