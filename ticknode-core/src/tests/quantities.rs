use crate::*;
use ticknode_shared::*;

fn add(e: &mut TickEngine, name: &str, props: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    e.create_node(id, name, props).unwrap();
    id
}
fn prop(k: &str, v: &str) -> (String, DataValue) {
    (k.into(), v.into())
}
fn pull(e: &mut TickEngine, id: NodeId) -> Option<DataValue> {
    e.pull_data(id, PinId(0), &mut Vec::new())
}
fn power() -> DataType {
    DataType::Named("quantity.power".into())
}

#[test]
fn format_node_uses_explicit_and_server_locales_and_persists_properties() {
    let mut e = TickEngine::new();
    assert_eq!(
        e.types.definitions().len(),
        ticknode_shared::quantity::catalog().len()
    );
    let id = add(
        &mut e,
        "text/to_text",
        &[prop("value_type", "quantity.power")],
    );
    e.set_input_value(id, PinId(1), Some(1970i64.into()))
        .unwrap();
    assert_eq!(pull(&mut e, id), Some("1.97 kW".into()));
    e.set_locale("uk");
    assert_eq!(pull(&mut e, id), Some("1,97 кВт".into()));
    e.set_node_property(id, "locale", "en".into()).unwrap();
    assert_eq!(pull(&mut e, id), Some("1.97 kW".into()));
    e.set_node_property(id, "unit", "watt".into()).unwrap();
    assert_eq!(pull(&mut e, id), Some("1970 W".into()));
    e.set_node_property(id, "digits", "2".into()).unwrap();
    e.set_node_property(id, "unit", "auto".into()).unwrap();
    assert_eq!(pull(&mut e, id), Some("2 kW".into()));
    let graph = e.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(pull(&mut restored, id), Some("2 kW".into()));
    restored
        .set_node_property(id, "value_type", "number".into())
        .unwrap();
    restored
        .set_node_property(id, "locale", "uk".into())
        .unwrap();
    restored
        .set_input_value(id, PinId(1), Some(1.975.into()))
        .unwrap();
    assert_eq!(pull(&mut restored, id), Some("2".into()));
}

struct Sink {
    id: NodeId,
    input: DataType,
}
impl Node for Sink {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        true
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/quantity_sink".into(),
            pins: vec![
                PinDefinition {
                    id: PinId(1),
                    name: "Value".into(),
                    direction: PinDirection::Input,
                    pin_type: PinType::Data(self.input.clone()),
                    required: true,
                },
                PinDefinition {
                    id: PinId(0),
                    name: "Result".into(),
                    direction: PinDirection::Output,
                    pin_type: PinType::Data(self.input.clone()),
                    required: false,
                },
            ],
            ..Default::default()
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        ctx.get_value(PinId(1))
            .map(|v| vec![NodeOutput::Data(PinId(0), v)])
            .unwrap_or_default()
    }
}
fn sink(e: &mut TickEngine, src: NodeId, input: DataType) -> NodeId {
    let id = NodeId::generate();
    e.add_node(Box::new(Sink { id, input }));
    e.connect_data(src, PinId(0), id, PinId(1)).unwrap();
    id
}
#[test]
fn wire_conversions_use_canonical_numbers_and_engine_locale() {
    let mut e = TickEngine::new();
    let p = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.power"),
            ("value".into(), 1970i64.into()),
        ],
    );
    assert_eq!(
        e.node_definition(p).unwrap().pins[0].pin_type,
        PinType::Data(power())
    );
    let s = sink(&mut e, p, DataType::String);
    assert_eq!(pull(&mut e, s), Some("1.97 kW".into()));
    e.set_locale("uk");
    assert_eq!(pull(&mut e, s), Some("1,97 кВт".into()));
    let n = sink(&mut e, p, DataType::Number);
    assert_eq!(pull(&mut e, n), Some(1970i64.into()));
    let c = add(&mut e, "const", &[("value".into(), 230i64.into())]);
    let q = sink(&mut e, c, power());
    assert_eq!(pull(&mut e, q), Some(230i64.into()));
    let text = add(&mut e, "const", &[("value".into(), "2,5 кВт".into())]);
    let parsed = sink(&mut e, text, power());
    assert_eq!(pull(&mut e, parsed), Some(2500i64.into()));
    let other = NodeId::generate();
    e.add_node(Box::new(Sink {
        id: other,
        input: DataType::Named("quantity.energy".into()),
    }));
    assert!(e.connect_data(p, PinId(0), other, PinId(1)).is_err());
}
#[test]
fn constant_and_format_node_adapt_and_keep_quantity_contract_after_reload() {
    let mut e = TickEngine::new();
    let c = add(&mut e, "const", &[]);
    assert!(e.adapt_node(c, PinId(0), &power()).is_ok());
    let f = add(&mut e, "text/to_text", &[]);
    let temp = DataType::Named("quantity.temperature".into());
    e.adapt_node(f, PinId(1), &temp).unwrap();
    assert_eq!(
        e.node_definition(f).unwrap().pins[0].pin_type,
        PinType::Data(temp)
    );
    let graph = e.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(
        restored.node_definition(c).unwrap().pins[0].pin_type,
        PinType::Data(power())
    );
}
#[test]
fn match_coerces_quantity_literals_through_registry() {
    let mut e = TickEngine::new();
    let list = |v: &str| DataValue::List {
        element_type: DataType::String,
        elements: vec![v.into()],
    };
    let id = add(
        &mut e,
        "match",
        &[
            ("inputs".into(), list("1:p:quantity.power")),
            ("cases".into(), list("1: p > 2 kW")),
        ],
    );
    for (value, pin) in [(2100, 1), (1900, 0)] {
        e.set_input_value(id, PinId(1), Some(DataValue::from(value as i64)))
            .unwrap();
        e.trigger_node(id, PinId(0));
        e.tick();
        assert_eq!(
            e.executed.iter().find(|t| t.node_id == id).unwrap().fired,
            vec![PinId(pin)]
        );
    }
}

#[test]
fn selecting_quantity_on_a_string_constant_keeps_its_output_numeric() {
    let mut e = TickEngine::new();
    let c = add(&mut e, "const", &[("value".into(), "2 kW".into())]);
    e.set_node_property(c, "value_type", "quantity.power".into())
        .unwrap();
    assert_eq!(pull(&mut e, c), Some(2000i64.into()));
    assert_eq!(
        e.node_definition(c).unwrap().pins[0].pin_type,
        PinType::Data(power())
    );
}

#[test]
fn to_text_adapts_on_connect_and_shares_live_display_rules_with_wires() {
    let mut e = TickEngine::new();
    let p = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.power"),
            ("value".into(), 5000i64.into()),
        ],
    );
    let node = add(&mut e, "text/to_text", &[]);
    e.connect_data(p, PinId(0), node, PinId(1)).unwrap();
    assert_eq!(
        e.node_definition(node).unwrap().pins[0].pin_type,
        PinType::Data(power())
    );
    let wire = sink(&mut e, p, DataType::String);
    assert_eq!(pull(&mut e, node), Some("5 kW".into()));
    let rule = QuantityDisplay {
        digits: 3,
        steps: vec![
            UnitStep {
                from: 0.0,
                unit: "watt".into(),
            },
            UnitStep {
                from: 10000.0,
                unit: "kilowatt".into(),
            },
        ],
        rounding: Default::default(),
    };
    e.set_display_rules(DisplayRules {
        quantities: [("quantity.power".into(), rule)].into(),
        ..Default::default()
    });
    for id in [node, wire] {
        assert_eq!(pull(&mut e, id), Some("5000 W".into()));
    }
    e.set_locale("uk");
    for id in [node, wire] {
        assert_eq!(pull(&mut e, id), Some("5000 Вт".into()));
    }
    e.set_node_property(node, "show_units", "no".into())
        .unwrap();
    assert_eq!(pull(&mut e, node), Some("5000".into()));
    e.set_node_property(node, "unit", "kilowatt".into())
        .unwrap();
    assert_eq!(pull(&mut e, node), Some("5".into()));
    // Connecting a different type replaces the adaptive contract, preserving hidden options.
    let boolean = add(&mut e, "const", &[("value".into(), true.into())]);
    e.connect_data(boolean, PinId(0), node, PinId(1)).unwrap();
    assert_eq!(pull(&mut e, node), Some("так".into()));
    e.set_node_property(node, "true_text", "on".into()).unwrap();
    assert_eq!(pull(&mut e, node), Some("on".into()));
    let graph = e.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(pull(&mut restored, node), Some("on".into()));
}

#[test]
fn snapshot_restoration_preserves_an_explicit_type_override_on_a_connected_to_text() {
    let mut e = TickEngine::new();
    let source = add(
        &mut e,
        "const",
        &[
            prop("value_type", "quantity.power"),
            ("value".into(), 1970i64.into()),
        ],
    );
    let node = add(&mut e, "text/to_text", &[]);
    e.connect_data(source, PinId(0), node, PinId(1)).unwrap();
    e.set_node_property(node, "value_type", "number".into())
        .unwrap();
    assert_eq!(pull(&mut e, node), Some("1970".into()));
    let graph = e.export_graph();
    let mut restored = TickEngine::new();
    assert!(restored.load_graph(&graph).is_empty());
    assert_eq!(restored.export_graph(), graph);
    assert_eq!(pull(&mut restored, node), Some("1970".into()));
}

#[path = "quantities_rounding.rs"]
mod rounding_tests;
