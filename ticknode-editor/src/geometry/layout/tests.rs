use super::*;
use ticknode_shared::{MIN_NODE_W, MAX_NODE_W, MAX_NODE_H, PropertyDefinition};

fn definition(name: &str) -> NodeDefinition {
    NodeDefinition {
        type_name: name.into(), title: name.into(), description: String::new(), help: String::new(),
        category: "Data".into(), properties: vec![], adaptive: false, cost: 1,
        pins: vec![
            PinDefinition { id: PinId(1), name: "Text".into(), direction: PinDirection::Input, pin_type: PinType::Data(DataType::String), required: false },
            PinDefinition { id: PinId(2), name: "Next".into(), direction: PinDirection::Input, pin_type: PinType::Exec, required: false },
            PinDefinition { id: PinId(3), name: "Out".into(), direction: PinDirection::Output, pin_type: PinType::Exec, required: false },
        ],
    }
}

#[test]
fn whole_extra_pixels_are_shared_and_last_growable_gets_the_remainder() {
    let def = definition("example");
    let props = vec![("first".into(), DataValue::from("one")), ("second".into(), DataValue::from("two"))];
    let natural = NodeLayout::new(&def, &props);
    let layout = NodeLayout::sized(&def, &props, Some(NodeSize { width: 500.0, height: natural.height + 101.0 }), &[]);
    assert_eq!(layout.width, 500.0);
    assert_eq!(layout.natural_height, natural.height);
    assert_eq!(layout.height, natural.height + 101.0);
    assert_eq!(layout.property_height("first"), ROW_H + 33.0);
    assert_eq!(layout.property_height("second"), ROW_H + 33.0);
    assert_eq!(layout.pin_height(PinId(1), PinDirection::Input), ROW_H + 35.0);
    let anchor = |l: &NodeLayout, id, dir| l.anchor(Position::default(), PinId(id), dir).unwrap();
    // Properties shift both columns; the grown input shifts only following inputs.
    assert_eq!(anchor(&layout, 1, PinDirection::Input).1, anchor(&natural, 1, PinDirection::Input).1 + 66.0);
    assert_eq!(anchor(&layout, 2, PinDirection::Input).1, anchor(&natural, 2, PinDirection::Input).1 + 101.0);
    assert_eq!(anchor(&layout, 3, PinDirection::Output), (500.0, anchor(&natural, 3, PinDirection::Output).1 + 66.0));
    assert_eq!(layout.pin_height(PinId(3), PinDirection::Output), ROW_H);
    assert_eq!(layout.detached_anchor(Position::default(), PinId(99), PinDirection::Input, 1, false).1,
        anchor(&layout, 2, PinDirection::Input).1);
}

#[test]
fn selects_pickers_types_connected_inputs_and_match_do_not_grow() {
    let mut def = definition("example");
    def.properties = vec![
        PropertyDefinition::select("select", "Select", &[]),
        PropertyDefinition::dynamic("picker", "Picker", "source"),
    ];
    let props = ["select", "picker", "element_type"].map(|s| (s.into(), DataValue::from("text")));
    let natural = NodeLayout::new(&def, &props);
    let size = Some(NodeSize { width: 320.0, height: natural.height + 120.0 });
    let layout = NodeLayout::sized(&def, &props, size, &[PinId(1)]);
    assert_eq!(layout.pins_top, natural.pins_top);
    assert_eq!(layout.inputs, natural.inputs);
    for name in ["select", "picker", "element_type"] { assert_eq!(layout.property_height(name), ROW_H); }
    assert_eq!(layout.height, size.unwrap().height);
    def.type_name = "match".into();
    let natural = NodeLayout::new(&def, &[]);
    let layout = NodeLayout::sized(&def, &[], Some(NodeSize { width: 160.0, height: 900.0 }), &[]);
    assert_eq!(layout.inputs, natural.inputs);
    assert_eq!(layout.outputs, natural.outputs);
    assert_eq!(layout.cases_top, natural.cases_top);
}

#[test]
fn automatic_layout_and_stored_bounds_respect_natural_height() {
    let def = definition("example");
    let natural = NodeLayout::new(&def, &[]);
    let auto = NodeLayout::sized(&def, &[], None, &[]);
    assert_eq!((auto.width, auto.height), (NODE_WIDTH, natural.height));
    assert_eq!(auto.inputs, natural.inputs);
    let small = NodeLayout::sized(&def, &[], Some(NodeSize { width: 1.0, height: 0.0 }), &[]);
    assert_eq!((small.width, small.height), (MIN_NODE_W, natural.height));
    let large = NodeLayout::sized(&def, &[], Some(NodeSize { width: 900.0, height: 2000.0 }), &[]);
    assert_eq!((large.width, large.height), (MAX_NODE_W, MAX_NODE_H));
}

#[test]
fn drags_snap_in_world_pixels_and_observe_all_bounds() {
    let origin = NodeSize { width: NODE_WIDTH, height: 100.0 };
    assert_eq!(resize_size(origin, (9.0, 9.0), 64.0), NodeSize { width: 248.0, height: 108.0 });
    assert_eq!(resize_size(origin, (-9999.0, -9999.0), 64.0), NodeSize { width: MIN_NODE_W, height: 64.0 });
    assert_eq!(resize_size(origin, (9999.0, 9999.0), 64.0), NodeSize { width: MAX_NODE_W, height: MAX_NODE_H });
}
