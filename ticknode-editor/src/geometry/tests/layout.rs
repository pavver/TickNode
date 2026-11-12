use super::*;

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
