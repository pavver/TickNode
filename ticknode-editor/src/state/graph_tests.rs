use super::*;

#[test]
fn required_input_presentation_tracks_server_values_bindings_and_connections() {
    let owner = Owner::new();
    owner.with(|| {
        let editor = Editor::new(Theme::Dark, RwSignal::new(Locale::En), true);
        let id = NodeId::generate();
        let pin = PinDefinition {
            id: PinId(1),
            name: "Value".into(),
            direction: PinDirection::Input,
            pin_type: PinType::Data(DataType::Number),
            required: true,
        };
        let instance = NodeInstance {
            model: NodeModel {
                id,
                type_name: "test/input".into(),
                properties: vec![],
                position: Position::default(),
                pins: None,
                size: None,
            },
            definition: NodeDefinition {
                type_name: "test/input".into(),
                pins: vec![pin.clone()],
                ..Default::default()
            },
            unavailable: None,
            frozen: vec![],
        };
        editor.nodes.update(|nodes| {
            nodes.insert(id, ClientNode::new(instance));
        });
        assert!(editor.required_missing(id, &pin));
        let node = editor.node_tracked(id).unwrap();
        node.instance.update(|instance| {
            instance
                .model
                .properties
                .push((input_variable_property(pin.id), "var.amount".into()))
        });
        assert!(!editor.required_missing(id, &pin));
        node.instance.update(|instance| {
            instance.model.properties = vec![(input_value_property(pin.id), 42i64.into())]
        });
        assert!(!editor.required_missing(id, &pin));
        node.instance
            .update(|instance| instance.model.properties.clear());
        editor.data.set(vec![ConnectionModel {
            src_node: NodeId::generate(),
            src_pin: PinId(0),
            dst_node: id,
            dst_pin: pin.id,
        }]);
        assert!(!editor.required_missing(id, &pin));
        editor.data.set(vec![]);
        assert!(editor.required_missing(id, &pin));
        let optional = PinDefinition {
            required: false,
            ..pin
        };
        assert!(!editor.required_missing(id, &optional));
    });
}
