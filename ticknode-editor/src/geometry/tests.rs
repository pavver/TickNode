use super::*;

#[test]
fn every_builtin_pin_color_is_stable() {
    for (ty, expected) in [
        (DataType::Bool, "var(--error)"),
        (DataType::Number, "var(--accent)"),
        (DataType::String, "var(--warning)"),
        (DataType::Time, "var(--info)"),
        (DataType::DateTime, "var(--info)"),
        (DataType::Duration, "var(--info)"),
        (DataType::Color, "var(--magenta)"),
        (DataType::List(Box::new(DataType::Any)), "var(--success)"),
        (DataType::Object, "var(--text-secondary)"),
        (DataType::Any, "var(--border-strong)"),
        (
            DataType::Named("test.value".into()),
            "var(--text-secondary)",
        ),
    ] {
        assert_eq!(data_type_color(&ty), expected);
    }
    assert_eq!(
        pin_color(&TypeRegistry::new(), &PinType::Exec),
        "var(--text-primary)"
    );
}

#[test]
fn match_inputs_skip_bad_entries_and_keep_their_order() {
    assert!(match_inputs_of(&[]).is_empty());
    assert!(match_inputs_of(&[("inputs".into(), DataValue::Bool(false))]).is_empty());
    let properties = [(
        "inputs".into(),
        DataValue::List {
            element_type: DataType::Any,
            elements: vec![
                DataValue::String("3:reading:number".into()),
                DataValue::Bool(true),
                DataValue::String("invalid".into()),
                DataValue::String("1:name:string".into()),
            ],
        },
    )];
    let inputs = match_inputs_of(&properties);
    assert_eq!(
        inputs
            .iter()
            .map(|i| (i.pin, i.name.as_str(), &i.value_type))
            .collect::<Vec<_>>(),
        [
            (3, "reading", &DataType::Number),
            (1, "name", &DataType::String)
        ]
    );
}

#[test]
fn split_fields_use_regular_rows_and_settings_add_no_property_row() {
    let field = ticknode_shared::split_field_pin(PinId(5), 0).unwrap();
    let next = ticknode_shared::split_field_pin(PinId(5), 1).unwrap();
    let mut def = NodeDefinition {
        adaptive: false,
        cost: 1,
        type_name: "example".into(),
        title: String::new(),
        category: String::new(),
        description: String::new(),
        help: String::new(),
        properties: vec![],
        pins: [field, next]
            .into_iter()
            .map(|id| PinDefinition {
                id,
                name: "State.field".into(),
                pin_type: PinType::Data(DataType::String),
                direction: PinDirection::Input,
                required: false,
            })
            .collect(),
    };
    let properties = vec![
        ("split:5".into(), DataValue::Bool(true)),
        (format!("in:{}", field.0), DataValue::from("value")),
    ];
    assert_eq!(prop_rows("match", &properties), 0);
    let layout = NodeLayout::new(&def, &properties);
    assert_eq!(
        layout.anchor(Position::default(), field, PinDirection::Input),
        Some((0.0, HEADER_H + BODY_PAD + ROW_H / 2.0))
    );
    assert_eq!(
        layout
            .anchor(Position::default(), next, PinDirection::Input)
            .unwrap()
            .1,
        HEADER_H + BODY_PAD + ROW_H * 1.5
    );
    assert_eq!(layout.height, HEADER_H + 2.0 * BODY_PAD + 2.0 * ROW_H);
    def.type_name = "match".into();
    let layout = NodeLayout::new(&def, &properties);
    assert_eq!(
        layout
            .anchor(Position::default(), field, PinDirection::Input)
            .unwrap()
            .1,
        HEADER_H + BODY_PAD + ROW_H * 1.5
    );
    assert_eq!(layout.cases_top, HEADER_H + BODY_PAD + 4.0 * ROW_H);
}

#[test]
fn zoom_keeps_cursor_point_fixed() {
    let view = Camera {
        x: 30.0,
        y: -20.0,
        zoom: 1.0,
    };
    let before = view.to_world(200.0, 150.0);
    let after = view.zoom_at(200.0, 150.0, 1.5).to_world(200.0, 150.0);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
}

fn def(type_name: &str, pins: Vec<(u32, PinType, PinDirection)>) -> NodeDefinition {
    NodeDefinition {
        adaptive: false,
        cost: 1,
        properties: Vec::new(),
        type_name: type_name.into(),
        title: "T".into(),
        category: "Data".into(),
        description: String::new(),
        help: String::new(),
        pins: pins
            .into_iter()
            .map(|(id, pin_type, direction)| PinDefinition {
                id: PinId(id),
                name: "p".into(),
                pin_type,
                direction,
                required: false,
            })
            .collect(),
    }
}

#[test]
fn property_rows_shift_pins() {
    use ticknode_shared::NumberValue;
    let d = def(
        "t",
        vec![(0, PinType::Data(DataType::Number), PinDirection::Output)],
    );
    let props = vec![
        ("value".to_string(), DataValue::Number(NumberValue::Int(1))),
        (
            "element_type".to_string(),
            DataValue::String("number".into()),
        ),
        ("in:1".to_string(), DataValue::Bool(true)),
    ];
    assert_eq!(prop_rows("example", &props), 2);
    let pos = Position { x: 0.0, y: 0.0 };
    let (_, y0) = NodeLayout::new(&d, &[])
        .anchor(pos, PinId(0), PinDirection::Output)
        .unwrap();
    let (_, y2) = NodeLayout::new(&d, &props)
        .anchor(pos, PinId(0), PinDirection::Output)
        .unwrap();
    assert_eq!(y2 - y0, 2.0 * ROW_H);
    assert_eq!(
        NodeLayout::new(&d, &props).height - NodeLayout::new(&d, &[]).height,
        2.0 * ROW_H
    );
    assert_eq!(NodeLayout::new(&d, &[]).height, MIN_NODE_HEIGHT);
}

#[test]
fn match_outputs_sit_on_case_headers() {
    let d = def(
        "match",
        vec![
            (0, PinType::Exec, PinDirection::Input),
            (1, PinType::Data(DataType::Any), PinDirection::Input),
            (3, PinType::Exec, PinDirection::Output),
            (7, PinType::Exec, PinDirection::Output),
            (0, PinType::Exec, PinDirection::Output),
        ],
    );
    let cases = DataValue::List {
        element_type: DataType::String,
        elements: vec![
            DataValue::String("3: a > 1 and a < 5".into()),
            DataValue::String("7:".into()),
        ],
    };
    let layout = NodeLayout::new(&d, &[("cases".into(), cases)]);
    let y = |pin| {
        layout
            .anchor(Position::default(), PinId(pin), PinDirection::Output)
            .unwrap()
            .1
    };
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
    types
        .insert_all(&[
            TypeDefinition::new_enum("p.colored", "C", &[("a", "A")]).colored("#e0a030"),
            TypeDefinition::new_enum("p.plain", "P", &[("a", "A")]),
            TypeDefinition::new_enum("p.evil", "E", &[("a", "A")])
                .colored("red;} body{display:none"),
        ])
        .unwrap();
    let color = |n: &str| type_color(&types, &DataType::Named(n.into()));
    assert_eq!(color("p.colored"), "#e0a030");
    assert!(color("p.plain").starts_with("hsl("));
    assert_eq!(color("p.plain"), color("p.plain"), "the color is stable");
    assert!(
        color("p.evil").starts_with("hsl("),
        "an invalid color does not reach the CSS"
    );
    assert_ne!(color("p.plain"), color("p.evil"));
    assert_eq!(type_color(&types, &DataType::Number), "var(--accent)");
}

#[test]
fn wire_conversion_matches_connection_rules() {
    let reg = TypeRegistry::new();
    let data = PinType::Data;
    assert_eq!(
        wire_conversion(&reg, &data(DataType::String), &data(DataType::Number)),
        Some(true)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Number), &data(DataType::Number)),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Any), &data(DataType::Number)),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &data(DataType::Object), &data(DataType::Number)),
        None
    );
    assert_eq!(
        wire_conversion(&reg, &PinType::Exec, &PinType::Exec),
        Some(false)
    );
    assert_eq!(
        wire_conversion(&reg, &PinType::Exec, &data(DataType::Number)),
        None
    );
}

#[test]
fn zoom_is_clamped() {
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 100.0).zoom, MAX_ZOOM);
    assert_eq!(Camera::default().zoom_at(0.0, 0.0, 0.001).zoom, MIN_ZOOM);
}

#[test]
fn routes_are_consecutive_cubics_and_empty_route_is_the_original_wire() {
    let a = (0.0, 0.0);
    let b = (400.0, 100.0);
    assert_eq!(route_path(a, &[], b), wire_path(a, b));
    assert_eq!(route_path(a, &[], b), "M 0 0 C 200 0, 200 100, 400 100");
    let points = [
        Position { x: 100.0, y: 80.0 },
        Position { x: 200.0, y: 40.0 },
    ];
    let path = route_path(a, &points, b);
    assert_eq!(path.matches('M').count(), 1);
    assert_eq!(path.matches('C').count(), 3);
    assert_eq!(
        path,
        "M 0 0 C 50 0, 50 80, 100 80 C 150 80, 150 40, 200 40 C 300 40, 300 100, 400 100"
    );
    assert_eq!(
        route_path((40.0, 80.0), &[], (0.0, 0.0)),
        "M 40 80 C 80 80, -40 0, 0 0"
    );
}

#[test]
fn insertion_uses_the_nearest_curve_segment_including_backwards_routes() {
    let points = [
        Position { x: 200.0, y: 0.0 },
        Position { x: 200.0, y: 200.0 },
    ];
    let a = (0.0, 0.0);
    let b = (400.0, 200.0);
    assert_eq!(route_insert_index(a, &points, b, (100.0, 0.0)), 0);
    assert_eq!(route_insert_index(a, &points, b, (200.0, 100.0)), 1);
    assert_eq!(route_insert_index(a, &points, b, (300.0, 200.0)), 2);
    assert_eq!(route_insert_index(a, &[], b, (-999.0, 999.0)), 0);
    // A right-to-left curve bows beyond its endpoints: insertion follows the curve.
    let points = [Position {
        x: -100.0,
        y: 100.0,
    }];
    assert_eq!(
        route_insert_index(a, &points, (100.0, 200.0), (-120.0, 98.0)),
        0
    );
    // Coincident endpoints do not divide by zero and tie-breaking is deterministic.
    assert_eq!(route_insert_index(a, &[Position::default()], a, a), 0);
}

#[test]
fn detached_anchor_uses_pin_then_clamped_directional_rows_and_outward_offset() {
    let layout = NodeLayout {
        pin_widths: vec![],
        width: NODE_WIDTH,
        height: 180.0,
        pins_top: 60.0,
        cases_top: 160.0,
        inputs: vec![(PinId(1), 72.0), (PinId(2), 96.0)],
        outputs: vec![(PinId(1), 144.0)],
        natural_height: 180.0,
        properties: vec![],
        input_heights: vec![],
    };
    let pos = Position { x: 100.0, y: 200.0 };
    // The pin ID and direction take precedence over its saved row.
    assert_eq!(
        layout.detached_anchor(pos, PinId(1), PinDirection::Input, 100, true),
        (68.0, 272.0)
    );
    assert_eq!(
        layout.detached_anchor(pos, PinId(1), PinDirection::Output, 0, true),
        (372.0, 344.0)
    );
    assert_eq!(
        layout.detached_anchor(pos, PinId(9), PinDirection::Input, 0, true),
        (68.0, 272.0)
    );
    assert_eq!(
        layout.detached_anchor(pos, PinId(9), PinDirection::Input, u32::MAX, true),
        (68.0, 296.0)
    );
    assert_eq!(
        layout.detached_anchor(pos, PinId(9), PinDirection::Output, 100, false),
        (340.0, 344.0)
    );
    let empty = NodeLayout {
        inputs: vec![],
        outputs: vec![],
        ..layout
    };
    assert_eq!(
        empty.detached_anchor(pos, PinId(9), PinDirection::Input, 99, true),
        (68.0, 260.0)
    );
    assert_eq!(
        empty.detached_anchor(pos, PinId(9), PinDirection::Output, 99, false),
        (340.0, 260.0)
    );
}

#[test]
fn wire_label_midpoint_follows_routed_curves_and_handles_overlapping_pins() {
    let a = (0.0, 0.0);
    let b = (400.0, 100.0);
    let middle = route_midpoint(a, &[], b);
    assert!((middle.0 - 200.0).abs() < 0.01 && (middle.1 - 50.0).abs() < 0.01);
    let route = [Position { x: 200.0, y: 300.0 }];
    let middle = route_midpoint(a, &route, (400.0, 0.0));
    assert!((middle.0 - 200.0).abs() < 0.01 && (middle.1 - 300.0).abs() < 0.01);
    let same = route_midpoint(a, &[], a);
    assert!(same.0.is_finite() && same.1.is_finite());
}

#[test]
fn to_text_hidden_properties_do_not_take_layout_rows() {
    let names = [
        "value_type",
        "unit",
        "rounding",
        "digits",
        "locale",
        "show_units",
        "true_text",
        "false_text",
        "datetime_format",
    ];
    for (ty, rows) in [
        ("any", 2),
        ("quantity.power", 6),
        ("bool", 4),
        ("datetime", 3),
        ("number", 4),
        ("duration", 3),
    ] {
        let props = names
            .iter()
            .map(|n| {
                (
                    n.to_string(),
                    DataValue::from(if *n == "value_type" { ty } else { "" }),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(prop_rows("text/to_text", &props), rows);
    }
}

#[test]
fn convert_hidden_type_and_irrelevant_units_take_no_layout_rows() {
    for (ty, target, rows) in [
        ("any", "", 1),
        ("number", "datetime", 1),
        ("duration", "seconds", 1),
        ("quantity.color_temperature", "color", 1),
        ("quantity.speed", "beaufort", 1),
        ("quantity.power", "number", 2),
        ("number", "quantity:quantity.color_temperature", 2),
        ("quantity.power", "invalid", 1),
    ] {
        let props = vec![
            ("value_type".into(), ty.into()),
            ("target".into(), target.into()),
            ("unit".into(), "kelvin".into()),
        ];
        assert!(!is_node_property(
            "convert",
            "value_type",
            &DataValue::from(ty)
        ));
        assert_eq!(prop_rows("convert", &props), rows, "{ty}: {target}");
    }
}

#[test]
fn hysteresis_type_is_hidden_while_mode_keeps_its_row() {
    let props = vec![
        ("value_type".into(), "quantity.voltage".into()),
        ("mode".into(), "rising".into()),
    ];
    assert!(!is_node_property(
        "logic/hysteresis",
        "value_type",
        &DataValue::from("quantity.voltage")
    ));
    assert_eq!(prop_rows("logic/hysteresis", &props), 1);
}

#[test]
fn template_node_includes_fixed_status_row_in_geometry() {
    let mut template_def = def(
        "text/template",
        vec![
            (1, PinType::Data(DataType::Any), PinDirection::Input),
            (0, PinType::Data(DataType::String), PinDirection::Output),
        ],
    );
    template_def.properties = vec![ticknode_shared::PropertyDefinition {
        name: "template".into(),
        title: "Template".into(),
        description: String::new(),
        kind: ticknode_shared::PropertyKind::Multiline,
        input: None,
    }];
    assert_eq!(status_rows(&template_def), 1);

    let regular_def = def(
        "text/concat",
        vec![
            (1, PinType::Data(DataType::Any), PinDirection::Input),
            (0, PinType::Data(DataType::String), PinDirection::Output),
        ],
    );
    assert_eq!(status_rows(&regular_def), 0);

    let props = vec![("template".into(), DataValue::from("{{ a }}"))];
    let template_layout = NodeLayout::new(&template_def, &props);
    let regular_layout = NodeLayout::new(&regular_def, &props);

    assert_eq!(template_layout.pins_top, HEADER_H + BODY_PAD + 2.0 * ROW_H);
    assert_eq!(regular_layout.pins_top, HEADER_H + BODY_PAD + 1.0 * ROW_H);
    assert_eq!(template_layout.pins_top - regular_layout.pins_top, ROW_H);

    let pos = Position { x: 0.0, y: 0.0 };
    let (_, t_y) = template_layout
        .anchor(pos, PinId(1), PinDirection::Input)
        .unwrap();
    let (_, r_y) = regular_layout
        .anchor(pos, PinId(1), PinDirection::Input)
        .unwrap();
    assert_eq!(t_y - r_y, ROW_H);

    let sized_template = NodeLayout::sized(&template_def, &props, None, &[]);
    assert_eq!(sized_template.pins_top, template_layout.pins_top);
    assert_eq!(sized_template.height, template_layout.height);
}
