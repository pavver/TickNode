use super::*;

#[test]
fn virtual_pin_ids_are_stable_distinct_and_bounded() {
    for parent in [0, 1, 255, 256, 0x3f_ffff] {
        for index in [0, 1, 255] {
            let id = split_field_pin(PinId(parent), index).unwrap();
            assert_ne!(id.0 & VIRTUAL_PIN_BIT, 0);
            assert_eq!(split_field_parent(id), Some((PinId(parent), index)));
            assert_eq!(
                parse_split_pin_property(&split_pin_property(PinId(parent))),
                Some(PinId(parent))
            );
        }
    }
    assert!(split_field_pin(PinId(0x40_0000), 0).is_none());
    assert!(split_field_pin(PinId(0), 256).is_none());
    assert!(split_field_pin(split_field_pin(PinId(0), 0).unwrap(), 0).is_none());
    assert!(split_field_parent(PinId(0x7fff_ffff)).is_none());
    assert!(split_field_parent(PinId(u32::MAX)).is_none());
    assert!(parse_split_pin_property("in:0").is_none());
}

#[test]
fn old_json_definition_defaults_to_native_cost() {
    let def: NodeDefinition = serde_json::from_str(
        r#"{
        "type_name":"old", "title":"Old", "category":"Test",
        "description":"", "pins":[]
    }"#,
    )
    .unwrap();
    assert_eq!(def.cost, 1);
    let mut json = serde_json::to_value(&def).unwrap();
    json["cost"] = 42.into();
    let def: NodeDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(def.cost, 42);
    assert_eq!(def.clone().cost(7).cost, 7);
}

fn data_pin(id: u32, data_type: DataType) -> PinDefinition {
    PinDefinition {
        id: PinId(id),
        name: "Value".into(),
        pin_type: PinType::Data(data_type),
        direction: PinDirection::Input,
        required: false,
    }
}

#[test]
fn required_pin_builder_preserves_the_rest_of_the_definition() {
    let pin = data_pin(7, DataType::Number);
    let required = pin.clone().required();
    assert!(required.required);
    assert_eq!(
        required,
        PinDefinition {
            required: true,
            ..pin
        }
    );
    let back: PinDefinition =
        postcard::from_bytes(&postcard::to_allocvec(&required).unwrap()).unwrap();
    assert!(back.required);
    assert_eq!(back, required);
}

#[test]
fn splitting_requires_nonempty_struct_and_representable_last_field() {
    for (count, id, expected) in [
        (0, 0, false),
        (1, 0, true),
        (255, 0x3f_ffff, true),
        (256, 0x3f_ffff, true),
        (257, 0, false),
        (1, 0x40_0000, false),
    ] {
        let definition = TypeDefinition::new_struct(
            "t.record",
            "Record",
            (0..count)
                .map(|i| {
                    crate::FieldDefinition::new(
                        &format!("f{i}"),
                        &format!("Field {i}"),
                        DataType::Number,
                    )
                })
                .collect(),
        );
        let types = TypeRegistry::from_definitions([definition.clone()]);
        let pin = data_pin(id, DataType::Named("t.record".into()));
        assert_eq!(
            splittable_type(&types, &pin),
            expected.then_some(&definition),
            "count={count}, id={id}"
        );
    }
}

#[test]
fn inline_values_require_editable_destination_scalar_source_and_compatible_type() {
    let types = TypeRegistry::new();
    let list = DataValue::List {
        element_type: DataType::Number,
        elements: vec![1i64.into()],
    };
    let object = DataValue::Object {
        fields: Default::default(),
        type_name: None,
    };
    for (destination, value, expected) in [
        (DataType::Number, DataValue::from(1i64), true),
        (DataType::Any, DataValue::Bool(false), true),
        (DataType::Number, DataValue::from("1"), false),
        (
            DataType::List(Box::new(DataType::Number)),
            list.clone(),
            false,
        ),
        (DataType::Object, object.clone(), false),
        (DataType::Any, list, false),
        (DataType::Any, object, false),
    ] {
        assert_eq!(
            input_value_fits(&types, &destination, &value),
            expected,
            "{value:?} -> {destination:?}"
        );
    }
}
