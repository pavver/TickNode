use super::*;
use ticknode_shared::{MAX_NODE_H, MAX_NODE_W, MIN_NODE_W, PropertyDefinition};

fn definition(name: &str) -> NodeDefinition {
    NodeDefinition {
        type_name: name.into(),
        title: name.into(),
        description: String::new(),
        help: String::new(),
        category: "Data".into(),
        properties: vec![],
        adaptive: false,
        cost: 1,
        pins: vec![
            PinDefinition {
                id: PinId(1),
                name: "Text".into(),
                direction: PinDirection::Input,
                pin_type: PinType::Data(DataType::String),
                required: false,
            },
            PinDefinition {
                id: PinId(2),
                name: "Next".into(),
                direction: PinDirection::Input,
                pin_type: PinType::Exec,
                required: false,
            },
            PinDefinition {
                id: PinId(3),
                name: "Out".into(),
                direction: PinDirection::Output,
                pin_type: PinType::Exec,
                required: false,
            },
        ],
    }
}

#[test]
fn whole_extra_pixels_are_shared_and_last_growable_gets_the_remainder() {
    let def = definition("example");
    let props = vec![
        ("first".into(), DataValue::from("one")),
        ("second".into(), DataValue::from("two")),
    ];
    let natural = NodeLayout::new(&def, &props);
    let layout = NodeLayout::sized(
        &def,
        &props,
        Some(NodeSize {
            width: 500.0,
            height: natural.height + 101.0,
        }),
        &[],
    );
    assert_eq!(layout.width, 500.0);
    assert_eq!(layout.natural_height, natural.height);
    assert_eq!(layout.height, natural.height + 101.0);
    assert_eq!(layout.property_height("first"), ROW_H + 33.0);
    assert_eq!(layout.property_height("second"), ROW_H + 33.0);
    assert_eq!(
        layout.pin_height(PinId(1), PinDirection::Input),
        ROW_H + 35.0
    );
    let anchor = |l: &NodeLayout, id, dir| l.anchor(Position::default(), PinId(id), dir).unwrap();
    // Properties shift both columns; the grown input shifts only following inputs.
    assert_eq!(
        anchor(&layout, 1, PinDirection::Input).1,
        anchor(&natural, 1, PinDirection::Input).1 + 66.0
    );
    assert_eq!(
        anchor(&layout, 2, PinDirection::Input).1,
        anchor(&natural, 2, PinDirection::Input).1 + 101.0
    );
    assert_eq!(
        anchor(&layout, 3, PinDirection::Output),
        (500.0, anchor(&natural, 3, PinDirection::Output).1 + 66.0)
    );
    assert_eq!(layout.pin_height(PinId(3), PinDirection::Output), ROW_H);
    assert_eq!(
        layout
            .detached_anchor(
                Position::default(),
                PinId(99),
                PinDirection::Input,
                1,
                false
            )
            .1,
        anchor(&layout, 2, PinDirection::Input).1
    );
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
    let size = Some(NodeSize {
        width: 320.0,
        height: natural.height + 120.0,
    });
    let layout = NodeLayout::sized(&def, &props, size, &[PinId(1)]);
    assert_eq!(layout.pins_top, natural.pins_top);
    assert_eq!(layout.inputs, natural.inputs);
    for name in ["select", "picker", "element_type"] {
        assert_eq!(layout.property_height(name), ROW_H);
    }
    assert_eq!(layout.height, size.unwrap().height);
    def.type_name = "match".into();
    let natural = NodeLayout::new(&def, &[]);
    let layout = NodeLayout::sized(
        &def,
        &[],
        Some(NodeSize {
            width: 160.0,
            height: 900.0,
        }),
        &[],
    );
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
    let small = NodeLayout::sized(
        &def,
        &[],
        Some(NodeSize {
            width: 1.0,
            height: 0.0,
        }),
        &[],
    );
    assert_eq!((small.width, small.height), (MIN_NODE_W, natural.height));
    let large = NodeLayout::sized(
        &def,
        &[],
        Some(NodeSize {
            width: 900.0,
            height: 2000.0,
        }),
        &[],
    );
    assert_eq!((large.width, large.height), (MAX_NODE_W, MAX_NODE_H));
}

#[test]
fn drags_snap_in_world_pixels_and_observe_all_bounds() {
    let origin = NodeSize {
        width: NODE_WIDTH,
        height: 100.0,
    };
    assert_eq!(
        resize_size(origin, (9.0, 9.0), 64.0),
        NodeSize {
            width: 248.0,
            height: 108.0
        }
    );
    assert_eq!(
        resize_size(origin, (-9999.0, -9999.0), 64.0),
        NodeSize {
            width: MIN_NODE_W,
            height: 64.0
        }
    );
    assert_eq!(
        resize_size(origin, (9999.0, 9999.0), 64.0),
        NodeSize {
            width: MAX_NODE_W,
            height: MAX_NODE_H
        }
    );
}

#[test]
fn quantity_constant_type_metadata_does_not_move_sockets_or_add_rows() {
    let def = definition("const");
    let plain = vec![("value".into(), DataValue::from(1970i64))];
    let typed = vec![
        ("value_type".into(), DataValue::from("quantity.power")),
        plain[0].clone(),
    ];
    let a = NodeLayout::new(&def, &plain);
    let b = NodeLayout::new(&def, &typed);
    assert_eq!(a, b);
    assert_eq!(
        NodeLayout::sized(&def, &plain, None, &[]),
        NodeLayout::sized(&def, &typed, None, &[])
    );
}

#[test]
fn schema_order_hidden_properties_and_described_types_share_layout() {
    let mut def = definition("example");
    def.properties = vec![
        PropertyDefinition::text("z", "Last alphabetically"),
        PropertyDefinition::text("event_type", "Event type"),
        PropertyDefinition::hidden("snapshot"),
        PropertyDefinition::multiline("template", "Template"),
    ];
    let mut props = [
        "a",
        "snapshot",
        "template",
        "event_type",
        "z",
        "element_type",
    ]
    .map(|n| (n.into(), DataValue::from("")));
    props[1].1 = DataValue::Bool(true);
    assert_eq!(
        property_rows(&def, &props)
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["z", "event_type", "template", "a", "element_type"]
    );
    assert!(!is_type_property(&def, "event_type"));
    assert!(is_type_property(&def, "element_type"));
    let natural = NodeLayout::new(&def, &props);
    assert_eq!(natural.pins_top, HEADER_H + BODY_PAD + 5.0 * ROW_H);
    let layout = NodeLayout::sized(
        &def,
        &props,
        Some(NodeSize {
            width: 400.0,
            height: natural.height + 40.0,
        }),
        &[PinId(1)],
    );
    assert_eq!(layout.property_height("template"), ROW_H + 10.0);
    assert_eq!(layout.property_height("event_type"), ROW_H + 10.0);
    assert_eq!(layout.property_height("element_type"), ROW_H);
    assert!(!layout.properties.iter().any(|(n, _)| n == "snapshot"));
}

#[test]
fn automatic_width_accounts_for_translated_labels_fields_and_both_pin_columns() {
    let mut def = definition("width/probe");
    def.properties = vec![PropertyDefinition::text("offset", "Offset")];
    let props = [("offset".into(), DataValue::from(""))];
    let plain = NodeLayout::sized(&def, &props, None, &[]);
    crate::i18n::set_runtime_translations(&[(
        "uk".into(),
        "node-width-probe-prop-offset-title = Зміщення відносно початку події".into(),
    )]);
    let translated = NodeLayout::sized(&def, &props, None, &[]);
    assert!(translated.width > plain.width);
    assert_eq!(
        translated
            .anchor(Position::default(), PinId(3), PinDirection::Output)
            .unwrap()
            .0,
        translated.width
    );
    crate::i18n::set_runtime_translations(&[]);
    def.pins[0].name = "A fairly long input label".into();
    def.pins[2].name = "Long output".into();
    let unconnected = NodeLayout::sized(&def, &[], None, &[]);
    let connected = NodeLayout::sized(&def, &[], None, &[PinId(1)]);
    assert!(unconnected.width > connected.width);
    assert!(unconnected.pin_width(PinId(1), PinDirection::Input) > 120.0);
    assert!(
        unconnected.pin_width(PinId(1), PinDirection::Input)
            + unconnected.pin_width(PinId(3), PinDirection::Output)
            <= unconnected.width
    );
    def.pins[0].name = "x".repeat(200);
    assert_eq!(NodeLayout::new(&def, &[]).width, 2.0 * NODE_WIDTH);
    assert_eq!(
        NodeLayout::sized(
            &def,
            &[],
            Some(NodeSize {
                width: 260.0,
                height: 100.0
            }),
            &[]
        )
        .width,
        260.0
    );
    def.type_name = "match".into();
    assert!(NodeLayout::new(&def, &[]).width >= MATCH_WIDTH);
}

#[test]
fn character_width_matches_css_font_size_and_bundled_font_metrics() {
    let css = include_str!("../../../styles/nodes.css");
    let prop_css = css
        .split(".prop-row {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    let size: f64 = prop_css
        .split("font-size:")
        .nth(1)
        .unwrap()
        .split("px")
        .next()
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let bytes = include_bytes!("../../../assets/CascadiaCode.ttf");
    let u16_at = |at| u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap());
    let u32_at = |at| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let table = |name: &[u8]| {
        (0..u16_at(4) as usize)
            .find_map(|i| {
                let at = 12 + i * 16;
                (&bytes[at..at + 4] == name).then(|| u32_at(at + 8))
            })
            .unwrap()
    };
    let em = u16_at(table(b"head") + 18);
    let advance = u16_at(table(b"hmtx"));
    assert_eq!(
        super::width::CHAR_WIDTH,
        size * f64::from(advance) / f64::from(em)
    );
    assert!(include_str!("../../../styles/base.css").contains("ui-monospace, monospace"));
}

#[test]
fn compact_select_leaves_room_for_full_ukrainian_label_and_text_fields_keep_minimum() {
    let mut def = definition("ha/action");
    def.pins.clear();
    def.properties = vec![PropertyDefinition::select(
        "skip_unchanged",
        "Skip if already set",
        &[("yes", "Yes"), ("no", "No")],
    )];
    let props = [("skip_unchanged".into(), "yes".into())];
    crate::i18n::set_runtime_translations(&[(
        "uk".into(),
        include_str!("../../../../ha-plugin/locales/uk.ftl").into(),
    )]);
    crate::i18n::with_locale(crate::i18n::Locale::Uk, || {
        let layout = NodeLayout::sized(&def, &props, None, &[]);
        let label = crate::i18n::property_title("ha/action", &def.properties[0]);
        assert_eq!(label, "Пропускати, якщо вже встановлено");
        let required = label.chars().count() as f64 * width::CHAR_WIDTH
            + 3.0 * width::CHAR_WIDTH
            + 24.0
            + 28.0;
        assert!(
            layout.width >= required,
            "the full label and compact select fit at automatic width"
        );
        assert!(
            layout.width < 400.0,
            "a two-choice selector does not reserve a text field"
        );
    });
    crate::i18n::set_runtime_translations(&[]);
    def.properties = vec![PropertyDefinition::text(
        "text",
        "A long label for a text field",
    )];
    let props = [("text".into(), "".into())];
    let layout = NodeLayout::new(&def, &props);
    assert!(layout.width >= 28.0 + 28.0 * width::CHAR_WIDTH + width::FIELD_MIN);
    let css = include_str!("../../../styles/nodes.css");
    assert!(css.contains("flex: 1 0 72px; min-width: 72px;"));
    assert!(css.contains(".prop-row .inline-value.select { flex: none; width: var(--selected-width, auto); min-width: 0; max-width: calc(var(--du) * 40); margin-left: auto; }"));
}

#[test]
fn connected_property_input_hides_row_in_height_width_and_resize_allocation() {
    let mut def = definition("input/probe");
    def.pins[0].name = "Target".into();
    def.properties =
        vec![PropertyDefinition::dynamic("target", &"Target ".repeat(15), "targets").or_input(1)];
    let props = [("target".into(), "".into())];
    assert_eq!(property_rows_connected(&def, &props, &[]).len(), 1);
    assert!(property_rows_connected(&def, &props, &[PinId(1)]).is_empty());
    let free = NodeLayout::sized(&def, &props, None, &[]);
    let wired = NodeLayout::sized(&def, &props, None, &[PinId(1)]);
    assert_eq!(free.pins_top - wired.pins_top, ROW_H);
    assert!(free.width > wired.width);
    let tall = NodeLayout::sized(
        &def,
        &props,
        Some(NodeSize {
            width: 480.0,
            height: 400.0,
        }),
        &[PinId(1)],
    );
    assert!(tall.properties.is_empty());
    assert_eq!(tall.pin_height(PinId(1), PinDirection::Input), ROW_H);
    let wide = NodeLayout::sized(
        &def,
        &props,
        Some(NodeSize {
            width: 480.0,
            height: 400.0,
        }),
        &[],
    );
    assert_eq!(
        wide.pin_height(PinId(1), PinDirection::Input),
        ROW_H,
        "a property input has no duplicate inline text field"
    );
    let small = NodeLayout::sized(
        &def,
        &props,
        Some(NodeSize {
            width: 300.0,
            height: 100.0,
        }),
        &[],
    );
    assert!(
        wide.pin_width(PinId(1), PinDirection::Input)
            > small.pin_width(PinId(1), PinDirection::Input)
    );
}

#[test]
fn enum_controls_reserve_translated_content_but_struct_inputs_have_no_field() {
    let enumeration = ticknode_shared::TypeDefinition::new_enum(
        "probe.mode",
        "Mode",
        &[("auto", "Automatic"), ("manual", "Manual")],
    );
    crate::i18n::set_known_types(&TypeRegistry::from_definitions(vec![enumeration]));
    crate::i18n::set_runtime_translations(&[(
        "uk".into(),
        "type-probe-mode-value-auto = Автоматичний режим\n".into(),
    )]);
    let mut def = definition("example");
    def.pins.truncate(1);
    def.pins[0].name = "A long input label".into();
    def.pins[0].pin_type = PinType::Data(DataType::Named("probe.mode".into()));
    crate::i18n::with_locale(crate::i18n::Locale::Uk, || {
        let props = [(
            ticknode_shared::input_value_property(PinId(1)),
            "auto".into(),
        )];
        let layout = NodeLayout::new(&def, &props);
        let content = ("A long input label".chars().count() + "Автоматичний режим".chars().count())
            as f64
            * width::CHAR_WIDTH
            + 24.0
            + 24.0
            + 16.0;
        assert!(layout.width >= content);
    });
    def.pins[0].pin_type = PinType::Data(DataType::Named("probe.struct".into()));
    assert_eq!(NodeLayout::new(&def, &[]).width, NODE_WIDTH);
    crate::i18n::set_runtime_translations(&[]);
    crate::i18n::set_known_types(&TypeRegistry::new());
}
