use super::*;

struct InputProperty {
    id: NodeId,
    target: String,
}
impl Node for InputProperty {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "probe/input_property".into(),
            title: "Input property".into(),
            description: String::new(),
            help: String::new(),
            category: "Test".into(),
            cost: 1,
            adaptive: false,
            properties: vec![
                PropertyDefinition::dynamic("target", "Target", "targets").or_input(1),
            ],
            pins: vec![PinDefinition {
                id: PinId(1),
                name: "Target".into(),
                direction: PinDirection::Input,
                pin_type: PinType::Data(DataType::String),
                required: true,
            }],
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("target".into(), self.target.clone().into())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "target"
            && let DataValue::String(text) = value
        {
            self.target = text;
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![]
    }
}
fn engine() -> TickEngine {
    let mut engine = TickEngine::new();
    engine.registry.register(Box::new(|id| {
        Box::new(InputProperty {
            id,
            target: String::new(),
        })
    }));
    engine
}

#[test]
fn selecting_property_removes_input_wire_and_inline_value_and_refreshes_cache() {
    let mut engine = engine();
    let source = NodeId::generate();
    let target = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), "light.hall".into())])
        .unwrap();
    engine
        .create_node(target, "probe/input_property", &[])
        .unwrap();
    assert!(
        engine
            .definition_arc(target)
            .unwrap()
            .pins
            .iter()
            .any(|p| p.id == PinId(1))
    );
    engine
        .set_input_value(target, PinId(1), Some("light.desk".into()))
        .unwrap();
    engine
        .connect_data(source, PinId(0), target, PinId(1))
        .unwrap();
    let removed = engine
        .set_node_property(target, "target", "light.kitchen".into())
        .unwrap();
    assert_eq!(removed.data.len(), 1);
    assert!(engine.data_connections().is_empty());
    assert!(engine.definition_arc(target).unwrap().pins.is_empty());
    assert!(
        !engine
            .node_model(target)
            .unwrap()
            .properties
            .iter()
            .any(|(name, _)| name == "in:1")
    );
    assert!(
        engine
            .connect_data(source, PinId(0), target, PinId(1))
            .is_err()
    );
    engine
        .set_node_property(target, "target", "".into())
        .unwrap();
    assert_eq!(engine.definition_arc(target).unwrap().pins.len(), 1);
    engine
        .connect_data(source, PinId(0), target, PinId(1))
        .unwrap();
    assert!(engine.frozen_changes(target).is_empty());
}

#[test]
fn legacy_property_and_connected_input_detach_visibly_and_survive_copy_and_restore() {
    let mut engine = engine();
    let source = NodeId::generate();
    let target = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), "light.hall".into())])
        .unwrap();
    engine
        .create_node(target, "probe/input_property", &[])
        .unwrap();
    engine
        .connect_data(source, PinId(0), target, PinId(1))
        .unwrap();
    let mut legacy = engine.export_graph();
    legacy
        .nodes
        .iter_mut()
        .find(|n| n.id == target)
        .unwrap()
        .properties[0]
        .1 = "light.kitchen".into();
    assert!(engine.load_graph(&legacy).is_empty());
    let saved = engine.export_graph();
    assert!(saved.data_connections.is_empty());
    assert_eq!(saved.detached_connections.len(), 1);
    assert_eq!(
        saved.detached_connections[0].connection,
        legacy.data_connections[0]
    );
    assert_eq!(saved.detached_connections[0].end, DetachedEnd::Target);
    assert!(engine.frozen_changes(target).is_empty());
    assert!(engine.load_graph(&saved).is_empty());
    assert_eq!(engine.export_graph(), saved);
    let (ids, warnings) = engine.import_snippet(&saved);
    assert!(warnings.is_empty());
    let new_target = ids.iter().find(|(old, _)| *old == target).unwrap().1;
    assert!(
        engine
            .export_graph()
            .detached_connections
            .iter()
            .any(|d| d.connection.dst_node == new_target)
    );
    engine
        .set_node_property(target, "target", "".into())
        .unwrap();
    engine
        .connect_data(source, PinId(0), target, PinId(1))
        .unwrap();
}
