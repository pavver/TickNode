use super::*;

#[test]
fn universal_nodes_have_identical_pins_to_per_type_nodes() {
    let mut engine = setup_engine();

    for struct_type in ["telegram.user", "telegram.message", "ha.state"] {
        let per_type_make = NodeId::generate();
        let per_type_break = NodeId::generate();
        let univ_make = NodeId::generate();
        let univ_break = NodeId::generate();

        engine
            .create_node(per_type_make, &format!("type/make/{struct_type}"), &[])
            .unwrap();
        engine
            .create_node(per_type_break, &format!("type/break/{struct_type}"), &[])
            .unwrap();
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

        let pt_make_def = engine.fresh_node_definition(per_type_make).unwrap();
        let univ_make_def = engine.fresh_node_definition(univ_make).unwrap();
        let pt_break_def = engine.fresh_node_definition(per_type_break).unwrap();
        let univ_break_def = engine.fresh_node_definition(univ_break).unwrap();

        // 1. Compare Make pins: pin 0 is Output Named(struct_type), pins 1..=N are field Inputs.
        assert_eq!(
            univ_make_def.pins.len(),
            pt_make_def.pins.len(),
            "Make pin count mismatch for {struct_type}"
        );
        for (u_pin, pt_pin) in univ_make_def.pins.iter().zip(pt_make_def.pins.iter()) {
            assert_eq!(u_pin.id, pt_pin.id, "Make pin id mismatch on {struct_type}");
            assert_eq!(
                u_pin.direction, pt_pin.direction,
                "Make pin direction mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
            assert_eq!(
                u_pin.pin_type, pt_pin.pin_type,
                "Make pin type mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
            assert_eq!(
                u_pin.required, pt_pin.required,
                "Make pin required mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
        }

        // 2. Compare Break pins: pin 0 is Input Named(struct_type), pins 1..=N are field Outputs.
        assert_eq!(
            univ_break_def.pins.len(),
            pt_break_def.pins.len(),
            "Break pin count mismatch for {struct_type}"
        );
        // Pin 0 contract:
        assert_eq!(univ_break_def.pins[0].id, PinId(0));
        assert_eq!(univ_break_def.pins[0].direction, PinDirection::Input);
        assert_eq!(
            univ_break_def.pins[0].pin_type,
            PinType::Data(DataType::Named(struct_type.to_string()))
        );
        assert!(univ_break_def.pins[0].required);

        // Field pins 1..=N match per-type break node identically:
        for (u_pin, pt_pin) in univ_break_def.pins[1..]
            .iter()
            .zip(pt_break_def.pins[1..].iter())
        {
            assert_eq!(
                u_pin.id, pt_pin.id,
                "Break pin id mismatch on {struct_type}"
            );
            assert_eq!(
                u_pin.direction, pt_pin.direction,
                "Break pin direction mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
            assert_eq!(
                u_pin.pin_type, pt_pin.pin_type,
                "Break pin type mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
            assert_eq!(
                u_pin.required, pt_pin.required,
                "Break pin required mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
            assert_eq!(
                u_pin.name, pt_pin.name,
                "Break pin name mismatch on {struct_type} pin {:?}",
                u_pin.id
            );
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
