use super::*;

#[test]
fn virtual_pin_ids_are_stable_distinct_and_bounded() {
    for parent in [0, 1, 255, 256, 0x3f_ffff] {
        for index in [0, 1, 255] {
            let id = split_field_pin(PinId(parent), index).unwrap();
            assert_ne!(id.0 & VIRTUAL_PIN_BIT, 0);
            assert_eq!(split_field_parent(id), Some((PinId(parent), index)));
            assert_eq!(parse_split_pin_property(&split_pin_property(PinId(parent))), Some(PinId(parent)));
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
    let def: NodeDefinition = serde_json::from_str(r#"{
        "type_name":"old", "title":"Old", "category":"Test",
        "description":"", "pins":[]
    }"#).unwrap();
    assert_eq!(def.cost, 1);
    let mut json = serde_json::to_value(&def).unwrap();
    json["cost"] = 42.into();
    let def: NodeDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(def.cost, 42);
    assert_eq!(def.clone().cost(7).cost, 7);
}
