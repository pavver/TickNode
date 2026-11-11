use super::*;

#[test]
fn legacy_digits_persist_significant_and_decimal_zero_survives_property_order() {
    let mut e = TickEngine::new();
    let legacy = add(
        &mut e,
        "text/to_text",
        &[prop("value_type", "number"), prop("digits", "2")],
    );
    e.set_input_value(legacy, PinId(1), Some(1.2345.into()))
        .unwrap();
    let graph = e.export_graph();
    let model = graph.nodes.iter().find(|n| n.id == legacy).unwrap();
    assert!(model.properties.contains(&prop("rounding", "significant")));
    let mut rules = DisplayRules::default();
    rules.number = NumberDisplay {
        rounding: Rounding::Decimals,
        digits: 4,
    };
    e.set_display_rules(rules.clone());
    assert_eq!(pull(&mut e, legacy), Some("1.2".into()));
    let project = add(&mut e, "text/to_text", &[prop("value_type", "number")]);
    e.set_input_value(project, PinId(1), Some(23i64.into()))
        .unwrap();
    assert_eq!(pull(&mut e, project), Some("23.0000".into()));
    for props in [
        vec![prop("digits", "0"), prop("rounding", "decimals")],
        vec![prop("rounding", "decimals"), prop("digits", "0")],
    ] {
        let node = add(&mut e, "text/to_text", &props);
        e.set_input_value(node, PinId(1), Some(23.5.into()))
            .unwrap();
        assert_eq!(pull(&mut e, node), Some("24".into()));
    }
    let graph = e.export_graph();
    let mut restored = TickEngine::new();
    restored.set_display_rules(rules);
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(pull(&mut restored, legacy), Some("1.2".into()));
    assert_eq!(pull(&mut restored, project), Some("23.0000".into()));
}

#[test]
fn currency_defaults_and_localized_labels_apply_to_nodes_and_converting_wires() {
    let mut e = TickEngine::new();
    let source = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.currency_uah"),
            ("value".into(), 10i64.into()),
        ],
    );
    let node = add(&mut e, "text/to_text", &[]);
    e.connect_data(source, PinId(0), node, PinId(1)).unwrap();
    let wire = sink(&mut e, source, DataType::String);
    for id in [node, wire] {
        assert_eq!(pull(&mut e, id), Some("10.00 UAH".into()));
    }
    e.set_locale("uk");
    for id in [node, wire] {
        assert_eq!(pull(&mut e, id), Some("10,00 грн".into()));
    }
    e.set_node_property(node, "locale", "en".into()).unwrap();
    e.set_node_property(node, "rounding", "significant".into())
        .unwrap();
    assert_eq!(pull(&mut e, node), Some("10 UAH".into()));

    for (name, value, en, uk) in [
        ("currency_jpy", 10.6, "11 JPY", "11 JPY"),
        ("currency_usd", 10.0, "10.00 USD", "10,00 дол."),
        ("beaufort", 5.6, "6 Bft", "6 Бфт"),
        ("aqi", 45.6, "46 AQI", "46 AQI"),
        ("ph", 7.16, "7.2 pH", "7,2 pH"),
    ] {
        let node = add(
            &mut e,
            "text/to_text",
            &[prop("value_type", &format!("quantity.{name}"))],
        );
        e.set_input_value(node, PinId(1), Some(value.into()))
            .unwrap();
        e.set_locale("en");
        assert_eq!(pull(&mut e, node), Some(en.into()));
        e.set_locale("uk");
        assert_eq!(pull(&mut e, node), Some(uk.into()));
    }
}

#[test]
fn a_currency_wire_cannot_connect_to_a_match_input_in_another_currency() {
    let mut e = TickEngine::new();
    let source = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.currency_uah"),
            ("value".into(), 10i64.into()),
        ],
    );
    let inputs = DataValue::List {
        element_type: DataType::String,
        elements: vec!["1:money:quantity.currency_usd".into()],
    };
    let node = add(&mut e, "match", &[("inputs".into(), inputs)]);
    assert!(e.connect_data(source, PinId(0), node, PinId(1)).is_err());
    e.set_node_property(source, "value_type", "quantity.currency_usd".into())
        .unwrap();
    e.connect_data(source, PinId(0), node, PinId(1)).unwrap();
}
