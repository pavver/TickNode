use super::*;

#[test]
fn universal_nodes_have_expected_pin_layout_for_struct_types() {
    let mut engine = setup_engine();

    for struct_type in ["telegram.user", "telegram.message", "ha.state"] {
        assert!(
            engine
                .registry
                .get_definition(&format!("type/make/{struct_type}"))
                .is_none()
        );
        assert!(
            engine
                .registry
                .get_definition(&format!("type/break/{struct_type}"))
                .is_none()
        );

        let fields = engine
            .types
            .get(struct_type)
            .unwrap_or_else(|| panic!("missing struct type {struct_type}"))
            .fields()
            .to_vec();

        let univ_make = NodeId::generate();
        let univ_break = NodeId::generate();

        engine
            .create_node(
                univ_make,
                "object/make",
                &[("type".into(), struct_type.into())],
            )
            .unwrap();
        engine
            .create_node(
                univ_break,
                "object/break",
                &[("type".into(), struct_type.into())],
            )
            .unwrap();

        let univ_make_def = engine.fresh_node_definition(univ_make).unwrap();
        let univ_break_def = engine.fresh_node_definition(univ_break).unwrap();

        // 1. Check Make pins: pin 0 is Output Named(struct_type), pins 1..=N are field Inputs.
        assert_eq!(
            univ_make_def.pins.len(),
            fields.len() + 1,
            "Make pin count mismatch for {struct_type}"
        );
        assert_eq!(univ_make_def.pins[0].id, PinId(0));
        assert_eq!(univ_make_def.pins[0].direction, PinDirection::Output);
        assert_eq!(
            univ_make_def.pins[0].pin_type,
            PinType::Data(DataType::Named(struct_type.to_string()))
        );
        assert!(!univ_make_def.pins[0].required);
        assert_eq!(univ_make_def.pins[0].name, "Value");

        for (i, field) in fields.iter().enumerate() {
            let pin = &univ_make_def.pins[i + 1];
            assert_eq!(pin.id, PinId(i as u32 + 1));
            assert_eq!(pin.direction, PinDirection::Input);
            assert_eq!(pin.pin_type, PinType::Data(field.data_type.clone()));
            assert_eq!(pin.required, !field.optional);
            let expected_name = if field.title.is_empty() {
                &field.name
            } else {
                &field.title
            };
            assert_eq!(&pin.name, expected_name);
        }

        // 2. Check Break pins: pin 0 is Input Named(struct_type), pins 1..=N are field Outputs.
        assert_eq!(
            univ_break_def.pins.len(),
            fields.len() + 1,
            "Break pin count mismatch for {struct_type}"
        );
        assert_eq!(univ_break_def.pins[0].id, PinId(0));
        assert_eq!(univ_break_def.pins[0].direction, PinDirection::Input);
        assert_eq!(
            univ_break_def.pins[0].pin_type,
            PinType::Data(DataType::Named(struct_type.to_string()))
        );
        assert!(univ_break_def.pins[0].required);
        assert_eq!(univ_break_def.pins[0].name, "Value");

        for (i, field) in fields.iter().enumerate() {
            let pin = &univ_break_def.pins[i + 1];
            assert_eq!(pin.id, PinId(i as u32 + 1));
            assert_eq!(pin.direction, PinDirection::Output);
            assert_eq!(pin.pin_type, PinType::Data(field.data_type.clone()));
            assert!(!pin.required);
            let expected_name = if field.title.is_empty() {
                &field.name
            } else {
                &field.title
            };
            assert_eq!(&pin.name, expected_name);
        }
    }
}

#[test]
fn unconfigured_universal_nodes_have_minimal_pins() {
    let mut engine = setup_engine();
    let brk = NodeId::generate();
    let make = NodeId::generate();

    engine.create_node(brk, "object/break", &[]).unwrap();
    engine.create_node(make, "object/make", &[]).unwrap();

    let brk_def = engine.fresh_node_definition(brk).unwrap();
    assert_eq!(brk_def.pins.len(), 1);
    assert_eq!(brk_def.pins[0].id, PinId(0));
    assert_eq!(brk_def.pins[0].name, "Value");
    assert_eq!(brk_def.pins[0].pin_type, PinType::Data(DataType::Any));
    assert_eq!(brk_def.pins[0].direction, PinDirection::Input);
    assert!(!brk_def.pins[0].required);

    let make_def = engine.fresh_node_definition(make).unwrap();
    assert_eq!(make_def.pins.len(), 1);
    assert_eq!(make_def.pins[0].id, PinId(0));
    assert_eq!(make_def.pins[0].name, "Value");
    assert_eq!(make_def.pins[0].pin_type, PinType::Data(DataType::Any));
    assert_eq!(make_def.pins[0].direction, PinDirection::Output);
    assert!(!make_def.pins[0].required);
}
