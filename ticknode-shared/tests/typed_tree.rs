use std::sync::Arc;
use ticknode_shared::{quantity::CanonicalLabels, template::*, *};

fn object(name: &str, value: DataValue, ty: Option<DataType>) -> DataValue {
    DataValue::Object {
        type_name: None,
        fields: [(
            name.into(),
            ObjectField {
                value,
                data_type: ty,
            },
        )]
        .into(),
    }
}
fn power() -> DataType {
    DataType::Named("quantity.power".into())
}
fn tree() -> DataValue {
    object(
        "attributes",
        object("power", 1200i64.into(), Some(power())),
        None,
    )
}
fn types() -> TypeRegistry {
    TypeRegistry::from_definitions(quantity::builtin_quantities())
}

#[test]
fn postcard_round_trips_optional_field_contracts_and_nested_lists() {
    for ty in [None, Some(power()), Some(DataType::List(Box::new(power())))] {
        let field = ObjectField {
            value: tree(),
            data_type: ty,
        };
        let bytes = postcard::to_allocvec(&field).unwrap();
        assert_eq!(postcard::from_bytes::<ObjectField>(&bytes).unwrap(), field);
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(serde_json::from_str::<ObjectField>(&json).unwrap(), field);
    }
}

#[test]
fn declarations_win_and_generic_fields_and_list_elements_use_their_contracts() {
    let mut types = types();
    types
        .insert_all(&[TypeDefinition::new_struct(
            "test.meter",
            "Meter",
            vec![FieldDefinition::new(
                "attributes",
                "Attributes",
                DataType::Object,
            )],
        )])
        .unwrap();
    assert_eq!(
        effective_type(
            Some(&tree()),
            &DataType::Named("test.meter".into()),
            "attributes.power",
            &types
        ),
        Some(power())
    );
    let list = DataValue::List {
        element_type: power(),
        elements: vec![1200i64.into()],
    };
    assert_eq!(
        effective_type(
            Some(&list),
            &DataType::List(Box::new(DataType::Any)),
            "0",
            &types
        ),
        Some(power())
    );
    let value = object("attributes", true.into(), Some(power()));
    let declared = TypeDefinition::new_struct(
        "test.fixed",
        "Fixed",
        vec![FieldDefinition::new(
            "attributes",
            "Attributes",
            DataType::Bool,
        )],
    );
    types.insert_all(&[declared]).unwrap();
    assert_eq!(
        effective_type(
            Some(&value),
            &DataType::Named("test.fixed".into()),
            "attributes",
            &types
        ),
        Some(DataType::Bool)
    );
}

#[test]
fn nested_text_unit_and_variable_paths_have_identical_presentation_and_keys() {
    let types = types();
    let input = tree();
    let source = "{{ text.value.attributes.power }}|{{ value.attributes.power | unit }}|{{ text.var.x.attributes.power }}|{{ var.x.attributes.power | unit }}|{{ text.ha.sensor.one.attributes.power }}|{{ ha.sensor.one.attributes.power | unit }}|{{ value | list | join(',') }}|{{ value.attributes | list | join(',') }}|{{ text.value }}";
    let format = |v: &DataValue, t: &DataType| {
        to_text::to_text_in(
            v,
            t,
            &Default::default(),
            &Default::default(),
            &CanonicalLabels,
            &types,
        )
    };
    let text = CompiledTemplate::new(source)
        .unwrap()
        .render_localized(
            &[("value".into(), Some(input.clone()), DataType::Any)],
            &format,
            Arc::new(text::format_value),
            RenderEnv {
                variables: vec![Variable {
                    key: VariableKey::global("x"),
                    data_type: DataType::Any,
                    value: input.clone(),
                }],
                namespaces: vec!["ha".into()],
                plugin_values: vec![("ha.sensor.one".into(), Some(input), DataType::Object)],
                to_text: Arc::new(|v, t, o| {
                    to_text::to_text(v, t, o, &Default::default(), &CanonicalLabels)
                }),
                ..Default::default()
            },
            &types,
        )
        .unwrap();
    assert_eq!(
        text,
        "1.2 kW|1.2 kW|1.2 kW|1.2 kW|1.2 kW|1.2 kW|attributes|power|{attributes: {power: 1.2 kW}}"
    );
}

#[test]
fn any_and_object_variable_round_trips_keep_declared_nested_types() {
    let mut types = types();
    types
        .insert_all(&[TypeDefinition::new_struct(
            "test.meter",
            "Meter",
            vec![FieldDefinition::new("power", "Power", power())],
        )])
        .unwrap();
    let mut input = object("power", 1200i64.into(), None);
    let DataValue::Object { type_name, .. } = &mut input else {
        unreachable!()
    };
    *type_name = Some("test.meter".into());
    for target in [DataType::Any, DataType::Object] {
        let stored = fit_typed_variable_value(
            &types,
            input.clone(),
            &DataType::Named("test.meter".into()),
            &target,
            &Default::default(),
            &CanonicalLabels,
        )
        .unwrap();
        let bytes = postcard::to_allocvec(&stored).unwrap();
        let stored = postcard::from_bytes::<DataValue>(&bytes).unwrap();
        assert_eq!(
            effective_type(Some(&stored), &target, "power", &types),
            Some(power())
        );
        let DataValue::Object { fields, .. } = &stored else {
            unreachable!()
        };
        assert_eq!(fields["power"].data_type, Some(power()));
    }
}

#[test]
fn identifiers_inside_untyped_objects_use_labels_and_completions_use_field_contracts() {
    let mut types = types();
    types
        .insert_all(&[TypeDefinition::identifier("test.id", "Device", "")])
        .unwrap();
    let value = object(
        "device",
        42i64.into(),
        Some(DataType::Named("test.id".into())),
    );
    assert_eq!(
        to_text::to_text_with_identifiers(
            &value,
            &DataType::Object,
            &Default::default(),
            &Default::default(),
            &CanonicalLabels,
            &types,
            &|_: &str, _: i64| Some("Kitchen".into())
        ),
        "{device: Kitchen}"
    );
    for source in ["{{ value.", "{{ text.value.", "{{ var.x.", "{{ text.var.x."] {
        let variables = vec![Variable {
            key: VariableKey::global("x"),
            data_type: DataType::Any,
            value: value.clone(),
        }];
        let input = CompletionInput {
            source,
            caret: source.len(),
            inputs: &[("value".into(), DataType::Any)],
            types: &types,
            now: None,
            labels: None,
            variables: &variables,
        };
        let result = complete::complete_with_values(
            &input,
            &[("value".into(), Some(value.clone()), DataType::Any)],
        )
        .unwrap();
        assert_eq!(result.items[0].label, "device");
        assert_eq!(result.items[0].detail, "Device");
        assert_eq!(
            result.items[0].help.as_ref().unwrap().key,
            "type-test-id-title"
        );
    }
}

#[test]
fn live_tree_paths_keep_contracts_beyond_the_declaration_expansion_limit() {
    let mut value = object("power", 1200i64.into(), Some(power()));
    for _ in 0..20 {
        value = object("nested", value, None);
    }
    let path = format!("value.{}power", "nested.".repeat(20));
    let (contracts, _) =
        unit::build_path_maps(&[("value".into(), Some(value), DataType::Object)], &types());
    assert_eq!(contracts[&path], power());
}

#[test]
fn static_list_indices_keep_element_units_in_text_unit_and_iteration() {
    let value = DataValue::List {
        element_type: power(),
        elements: vec![1200i64.into()],
    };
    let input = object("items", value, None);
    let format = |v: &DataValue, ty: &DataType| {
        to_text::to_text(
            v,
            ty,
            &Default::default(),
            &Default::default(),
            &CanonicalLabels,
        )
    };
    let source = "{{ text.value.items[0] }}|{{ value.items[0] | unit }}|{% for item in text.value.items %}{{ item }}{% endfor %}";
    let output = CompiledTemplate::new(source)
        .unwrap()
        .render_localized(
            &[("value".into(), Some(input), DataType::Object)],
            &format,
            Arc::new(text::format_value),
            RenderEnv::default(),
            &types(),
        )
        .unwrap();
    assert_eq!(output, "1.2 kW|1.2 kW|1.2 kW");
}
