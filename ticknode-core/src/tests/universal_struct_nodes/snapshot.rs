use super::*;

#[test]
fn snapshot_json_roundtrips() {
    let fields = vec![
        FieldDefinition::new("a", "Alpha", DataType::Number),
        FieldDefinition::new("b", "Beta", DataType::String).optional(),
    ];
    let encoded = encode_fields_snapshot("Custom Struct", &fields).expect("encoded json");
    let decoded = decode_fields_snapshot(&encoded).expect("decoded snapshot");
    assert_eq!(decoded.title, "Custom Struct");
    assert_eq!(decoded.fields, fields);
}

#[test]
fn corrupt_fields_property_is_ignored() {
    let mut engine = TickEngine::new();
    let make = NodeId::generate();
    let brk = NodeId::generate();

    // Create with corrupt fields property before or after type
    engine
        .create_node(
            make,
            "object/make",
            &[
                ("type".into(), "unregistered.type".into()),
                ("fields".into(), "not-valid-json".into()),
            ],
        )
        .unwrap();
    engine
        .create_node(
            brk,
            "object/break",
            &[
                ("fields".into(), r#"{"title": 42}"#.into()),
                ("type".into(), "unregistered.type".into()),
            ],
        )
        .unwrap();

    let make_def = engine.fresh_node_definition(make).unwrap();
    let brk_def = engine.fresh_node_definition(brk).unwrap();
    // Since corrupt JSON is ignored and type is unregistered, no field pins exist
    assert_eq!(make_def.pins.len(), 1); // Value out
    assert_eq!(brk_def.pins.len(), 1); // Value in
}

#[test]
fn interactive_fields_edit_is_rejected() {
    let mut engine = setup_engine();
    let make = NodeId::generate();
    let brk = NodeId::generate();

    engine
        .create_node(make, "object/make", &[("type".into(), "ha.state".into())])
        .unwrap();
    engine
        .create_node(brk, "object/break", &[("type".into(), "ha.state".into())])
        .unwrap();

    let err_make = engine
        .set_node_property(make, "fields", r#"{"title":"X","fields":[]}"#.into())
        .unwrap_err();
    assert_eq!(err_make.ui_message().key, "error-invalid-property");

    let err_brk = engine
        .set_node_property(brk, "fields", r#"{"title":"X","fields":[]}"#.into())
        .unwrap_err();
    assert_eq!(err_brk.ui_message().key, "error-invalid-property");
}

#[test]
fn order_independence_for_unregistered_type() {
    let fields = vec![
        FieldDefinition::new("x", "X Coord", DataType::Number),
        FieldDefinition::new("y", "Y Coord", DataType::Number),
    ];
    let snapshot_json = encode_fields_snapshot("Point2D", &fields).unwrap();

    // 1. type then fields
    let mut engine1 = TickEngine::new();
    let n1 = NodeId::generate();
    engine1
        .create_node(
            n1,
            "object/make",
            &[
                ("type".into(), "geometry.point".into()),
                ("fields".into(), snapshot_json.clone().into()),
            ],
        )
        .unwrap();

    // 2. fields then type
    let mut engine2 = TickEngine::new();
    let n2 = NodeId::generate();
    engine2
        .create_node(
            n2,
            "object/make",
            &[
                ("fields".into(), snapshot_json.clone().into()),
                ("type".into(), "geometry.point".into()),
            ],
        )
        .unwrap();

    let def1 = engine1.fresh_node_definition(n1).unwrap();
    let def2 = engine2.fresh_node_definition(n2).unwrap();

    assert_eq!(def1.pins, def2.pins);
    assert_eq!(def1.properties, def2.properties);
    assert_eq!(def1.pins.len(), 3); // Value (out) + 2 fields (in)
    assert_eq!(def1.pins[1].name, "X Coord");
    assert_eq!(def1.pins[2].name, "Y Coord");
}

#[test]
fn live_registered_type_always_wins_over_stored_snapshot() {
    let old_fields = vec![FieldDefinition::new(
        "old_field",
        "Old Field",
        DataType::String,
    )];
    let old_snapshot_json = encode_fields_snapshot("Old State Title", &old_fields).unwrap();

    // Create engine with live ha.state type
    let mut engine = setup_engine();
    let make = NodeId::generate();
    let brk = NodeId::generate();

    // Load with old snapshot in stored properties
    engine
        .create_node(
            make,
            "object/make",
            &[
                ("type".into(), "ha.state".into()),
                ("fields".into(), old_snapshot_json.clone().into()),
            ],
        )
        .unwrap();
    engine
        .create_node(
            brk,
            "object/break",
            &[
                ("fields".into(), old_snapshot_json.into()),
                ("type".into(), "ha.state".into()),
            ],
        )
        .unwrap();

    let make_def = engine.fresh_node_definition(make).unwrap();
    let brk_def = engine.fresh_node_definition(brk).unwrap();

    // ha.state has 5 fields in setup_engine(), so pins = 6 (1 value + 5 fields), NOT 2 (old snapshot)
    assert_eq!(make_def.pins.len(), 6);
    assert_eq!(brk_def.pins.len(), 6);
    assert_eq!(make_def.pins[1].name, "Entity ID");
    assert_eq!(brk_def.pins[1].name, "Entity ID");

    // The exported properties hold the fresh live snapshot, not the old one
    let make_props = engine.nodes.get(&make).unwrap().properties();
    let brk_props = engine.nodes.get(&brk).unwrap().properties();

    let make_fields_val = make_props.iter().find(|(k, _)| k == "fields").unwrap();
    let DataValue::String(json_str) = &make_fields_val.1 else {
        panic!("expected string");
    };
    let decoded = decode_fields_snapshot(json_str).unwrap();
    assert_eq!(decoded.title, "HA State");
    assert_eq!(decoded.fields.len(), 5);

    let brk_fields_val = brk_props.iter().find(|(k, _)| k == "fields").unwrap();
    let DataValue::String(json_str_brk) = &brk_fields_val.1 else {
        panic!("expected string");
    };
    let decoded_brk = decode_fields_snapshot(json_str_brk).unwrap();
    assert_eq!(decoded_brk.title, "HA State");
    assert_eq!(decoded_brk.fields.len(), 5);
}

#[test]
fn changing_type_interactively_replaces_or_clears_snapshot() {
    let mut engine = setup_engine();
    let make = NodeId::generate();
    engine
        .create_node(make, "object/make", &[("type".into(), "ha.state".into())])
        .unwrap();

    let props_before = engine.nodes.get(&make).unwrap().properties();
    assert!(props_before.iter().any(|(k, _)| k == "fields"));

    // Change to telegram.user: replaces snapshot with telegram.user
    engine
        .set_node_property(make, "type", "telegram.user".into())
        .unwrap();
    let props_mid = engine.nodes.get(&make).unwrap().properties();
    let fields_mid = props_mid.iter().find(|(k, _)| k == "fields").unwrap();
    let DataValue::String(json_mid) = &fields_mid.1 else {
        panic!("expected string");
    };
    let decoded_mid = decode_fields_snapshot(json_mid).unwrap();
    assert_eq!(decoded_mid.title, "Telegram user");
    assert_eq!(decoded_mid.fields.len(), 6);

    // Change to empty: clears snapshot completely
    engine.set_node_property(make, "type", "".into()).unwrap();
    let props_after = engine.nodes.get(&make).unwrap().properties();
    assert!(!props_after.iter().any(|(k, _)| k == "fields"));
}
