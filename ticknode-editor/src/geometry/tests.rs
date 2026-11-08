use super::*;

#[test]
fn split_fields_use_regular_rows_and_settings_add_no_property_row() {
    let field = ticknode_shared::split_field_pin(PinId(5), 0).unwrap();
    let next = ticknode_shared::split_field_pin(PinId(5), 1).unwrap();
    let mut def = NodeDefinition {
        type_name: "example".into(), title: String::new(), category: String::new(),
        description: String::new(), help: String::new(), properties: vec![],
        pins: [field, next].into_iter().map(|id| PinDefinition {
            id, name: "State.field".into(), pin_type: PinType::Data(DataType::String),
            direction: PinDirection::Input, required: false,
        }).collect(),
    };
    let properties = vec![("split:5".into(), DataValue::Bool(true)), (format!("in:{}", field.0), DataValue::from("value"))];
    assert_eq!(prop_rows(&properties), 0);
    let layout = NodeLayout::new(&def, &properties);
    assert_eq!(layout.anchor(Position::default(), field, PinDirection::Input), Some((0.0, HEADER_H + BODY_PAD + ROW_H / 2.0)));
    assert_eq!(layout.anchor(Position::default(), next, PinDirection::Input).unwrap().1, HEADER_H + BODY_PAD + ROW_H * 1.5);
    assert_eq!(layout.height, HEADER_H + 2.0 * BODY_PAD + 2.0 * ROW_H);
    def.type_name = "match".into();
    let layout = NodeLayout::new(&def, &properties);
    assert_eq!(layout.anchor(Position::default(), field, PinDirection::Input).unwrap().1, HEADER_H + BODY_PAD + ROW_H * 1.5);
    assert_eq!(layout.cases_top, HEADER_H + BODY_PAD + 4.0 * ROW_H);
}

#[test]
fn zoom_keeps_cursor_point_fixed() {
    let view = Camera { x: 30.0, y: -20.0, zoom: 1.0 };
    let before = view.to_world(200.0, 150.0);
    let after = view.zoom_at(200.0, 150.0, 1.5).to_world(200.0, 150.0);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
}

fn def(type_name: &str, pins: Vec<(u32, PinType, PinDirection)>) -> NodeDefinition {
    NodeDefinition {
        properties: Vec::new(),
        type_name: type_name.into(), title: "T".into(), category: "Data".into(), description: String::new(), help: String::new(),
        pins: pins.into_iter().map(|(id, pin_type, direction)| PinDefinition { id: PinId(id), name: "p".into(), pin_type, direction, required: false }).collect(),
    }
}

#[test]
fn property_rows_shift_pins() {
    use ticknode_shared::NumberValue;
    let d = def("t", vec![(0, PinType::Data(DataType::Number), PinDirection::Output)]);
    let props = vec![
        ("value".to_string(), DataValue::Number(NumberValue::Int(1))),
        ("element_type".to_string(), DataValue::String("number".into())),
        ("in:1".to_string(), DataValue::Bool(true)),
    ];
    assert_eq!(prop_rows(&props), 2);
    let pos = Position { x: 0.0, y: 0.0 };
    let (_, y0) = NodeLayout::new(&d, &[]).anchor(pos, PinId(0), PinDirection::Output).unwrap();
    let (_, y2) = NodeLayout::new(&d, &props).anchor(pos, PinId(0), PinDirection::Output).unwrap();
    assert_eq!(y2 - y0, 2.0 * ROW_H);
    assert_eq!(NodeLayout::new(&d, &props).height - NodeLayout::new(&d, &[]).height, 2.0 * ROW_H);
    assert_eq!(NodeLayout::new(&d, &[]).height, MIN_NODE_HEIGHT);
}

#[test]
fn match_outputs_sit_on_case_headers() {
    let d = def("match", vec![
        (0, PinType::Exec, PinDirection::Input),
        (1, PinType::Data(DataType::Any), PinDirection::Input),
        (3, PinType::Exec, PinDirection::Output),
        (7, PinType::Exec, PinDirection::Output),
        (0, PinType::Exec, PinDirection::Output),
    ]);
    let cases = DataValue::List { element_type: DataType::String, elements: vec![
        DataValue::String("3: a > 1 and a < 5".into()),
        DataValue::String("7:".into()),
    ] };
    let layout = NodeLayout::new(&d, &[("cases".into(), cases)]);
    let y = |pin| layout.anchor(Position::default(), PinId(pin), PinDirection::Output).unwrap().1;
    let top = layout.cases_top;
    // In, input, "+ input"
    assert_eq!(top, layout.pins_top + 3.0 * ROW_H);
    assert_eq!(y(3), top + ROW_H / 2.0);
    // Case 3: header + 2 conditions; case 7: header + "always"
    assert_eq!(y(7), top + 3.0 * ROW_H + ROW_H / 2.0);
    assert_eq!(y(0), top + 5.0 * ROW_H + ROW_H / 2.0);
    assert_eq!(layout.height, top + 6.0 * ROW_H + BODY_PAD);
}

#[test]
fn named_type_colors() {
    use ticknode_shared::TypeDefinition;
    let mut types = TypeRegistry::new();
    types.insert_all(&[
        TypeDefinition::new_enum("p.colored", "C", &[("a", "A")]).colored("#e0a030"),
        TypeDefinition::new_enum("p.plain", "P", &[("a", "A")]),
        TypeDefinition::new_enum("p.evil", "E", &[("a", "A")]).colored("red;} body{display:none"),
    ]).unwrap();
    let color = |n: &str| type_color(&types, &DataType::Named(n.into()));
    assert_eq!(color("p.colored"), "#e0a030");
    assert!(color("p.plain").starts_with("hsl("));
    assert_eq!(color("p.plain"), color("p.plain"), "the color is stable");
    assert!(color("p.evil").starts_with("hsl("), "an invalid color does not reach the CSS");
    assert_ne!(color("p.plain"), color("p.evil"));
    assert_eq!(type_color(&types, &DataType::Number), "var(--accent)");
}

#[test]
fn wire_conversion_matches_connection_rules() {
    let reg = TypeRegistry::new();
    let data = PinType::Data;
    assert_eq!(wire_conversion(&reg, &data(DataType::String), &data(DataType::Number)), Some(true));
    assert_eq!(wire_conversion(&reg, &data(DataType::Number), &data(DataType::Number)), Some(false));
    assert_eq!(wire_conversion(&reg, &data(DataType::Any), &data(DataType::Number)), Some(false));
    assert_eq!(wire_conversion(&reg, &data(DataType::Object), &data(DataType::Number)), None);
    assert_eq!(wire_conversion(&reg, &PinType::Exec, &PinType::Exec), Some(false));
    assert_eq!(wire_conversion(&reg, &PinType::Exec, &data(DataType::Number)), None);
}

#[test]
fn zoom_is_clamped() {
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 100.0).zoom, MAX_ZOOM);
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 0.001).zoom, MIN_ZOOM);
}
