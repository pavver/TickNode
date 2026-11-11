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
