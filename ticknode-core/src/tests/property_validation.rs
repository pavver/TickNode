use super::*;

struct PropertyProbe {
    id: NodeId,
    value: DataValue,
}
impl Node for PropertyProbe {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "probe/property".into(),
            title: "Probe".into(),
            description: String::new(),
            help: String::new(),
            category: "Test".into(),
            cost: 1,
            adaptive: false,
            pins: vec![],
            properties: vec![PropertyDefinition::duration("duration", "Duration", true)],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("duration".into(), self.value.clone())]
    }
    fn set_property(&mut self, _: &str, value: DataValue) {
        self.value = value;
    }
    fn validate_property(&self, _: &str, value: &DataValue) -> Result<(), UiMessage> {
        if *value == DataValue::from("10s") {
            Err(UiMessage::new("probe-rejected", "Rejected by node").arg("value", "10s"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn interactive_validation_is_atomic_and_restoration_bypasses_it() {
    let mut engine = TickEngine::new();
    engine.registry.register(Box::new(|id| {
        Box::new(PropertyProbe {
            id,
            value: "1s".into(),
        })
    }));
    let id = NodeId::generate();
    engine.create_node(id, "probe/property", &[]).unwrap();
    let before = engine.export_graph();
    for (text, key) in [
        ("abc", "error-property-duration"),
        ("10s", "probe-rejected"),
    ] {
        let error = engine
            .set_node_property(id, "duration", text.into())
            .unwrap_err();
        assert!(matches!(&error, EngineError::InvalidProperty(_)));
        assert_eq!(error.ui_message().key, key);
        assert_eq!(engine.export_graph(), before);
    }
    engine
        .set_node_property(id, "duration", "-01:00:00".into())
        .unwrap();
    let mut stored = before;
    stored.nodes[0].properties[0].1 = "10s".into();
    assert!(engine.load_graph(&stored).is_empty());
    assert_eq!(
        engine.node_model(id).unwrap().properties,
        stored.nodes[0].properties
    );
    stored.nodes[0].properties[0].1 = "legacy-invalid".into();
    assert!(engine.reconcile(&stored).warnings.is_empty());
    assert_eq!(
        engine.node_model(id).unwrap().properties,
        stored.nodes[0].properties
    );
    assert!(engine.import_snippet(&stored).1.is_empty());
}

#[test]
fn interactive_creation_keeps_initial_property_restoration_semantics() {
    let mut engine = TickEngine::new();
    let properties = vec![(
        "cases".into(),
        DataValue::List {
            element_type: DataType::String,
            elements: vec!["2: a > 10".into()],
        },
    )];
    let restored = NodeId::generate();
    let interactive = NodeId::generate();
    engine.create_node(restored, "match", &properties).unwrap();
    engine
        .create_node_interactive(interactive, "match", &properties)
        .unwrap();
    let expected = engine.node_definition(restored).unwrap();
    assert!(expected.pins.iter().any(|p| p.name == "a"));
    assert_eq!(engine.node_definition(interactive).unwrap(), expected);
    assert_eq!(
        engine.node_model(interactive).unwrap().properties,
        engine.node_model(restored).unwrap().properties
    );
}
