use super::*;

#[test]
fn unknown_graph_nodes_keep_inferred_pin_types_order_and_fanout() {
    let mut e = TickEngine::new();
    let pins = vec![
        pin(7, PinDirection::Input, DataType::String),
        pin(7, PinDirection::Output, DataType::Number),
        pin(8, PinDirection::Input, DataType::Bool),
    ];
    let a = fixed(
        &mut e,
        pins.clone(),
        vec![NodeOutput::Data(PinId(7), 13i64.into())],
    );
    let b = fixed(&mut e, pins, vec![NodeOutput::Data(PinId(7), 13i64.into())]);
    let button = add(&mut e, "test/click_button");
    let console = add(&mut e, "console");
    let unknown = NodeId::generate();
    let mut graph = e.export_graph();
    graph.nodes.push(NodeModel {
        id: unknown,
        type_name: "missing/node".into(),
        properties: vec![("saved".into(), "kept".into())],
        position: Position::default(),
        pins: None,
        size: None,
    });
    let connection = |src_node, src_pin, dst_node, dst_pin| ConnectionModel {
        src_node,
        src_pin: PinId(src_pin),
        dst_node,
        dst_pin: PinId(dst_pin),
    };
    graph.exec_connections = vec![
        connection(button, 0, unknown, 0),
        connection(unknown, 0, console, 0),
    ];
    graph.data_connections = vec![
        connection(a, 7, unknown, 3),
        connection(unknown, 4, a, 7),
        connection(unknown, 4, b, 7),
        connection(unknown, 6, b, 8),
    ];
    // The feedback through a would form a data cycle; use a separate source for the incoming edge.
    let source = add(&mut e, "const");
    graph.nodes.push(e.node_model(source).unwrap());
    graph.data_connections[0].src_node = source;
    graph.data_connections[0].src_pin = PinId(0);
    assert!(e.load_graph(&graph).is_empty());
    let def = e.node_definition(unknown).unwrap();
    let signature: Vec<_> = def
        .pins
        .iter()
        .map(|p| (p.id, p.direction, p.pin_type.clone(), p.required))
        .collect();
    assert_eq!(
        signature,
        [
            (PinId(0), PinDirection::Input, PinType::Exec, false),
            (
                PinId(3),
                PinDirection::Input,
                PinType::Data(DataType::Number),
                false
            ),
            (PinId(0), PinDirection::Output, PinType::Exec, false),
            (
                PinId(4),
                PinDirection::Output,
                PinType::Data(DataType::String),
                false
            ),
            (
                PinId(6),
                PinDirection::Output,
                PinType::Data(DataType::Bool),
                false
            ),
        ]
    );
    assert_eq!(e.exec_connections().len(), 2);
    assert_eq!(e.data_connections().len(), 4);
    assert_eq!(
        e.node_model(unknown).unwrap().properties,
        [("saved".into(), "kept".into())]
    );
    // Triggering a stub answers with its unavailable error, even with absent connected data.
    e.trigger_node(unknown, PinId(0));
    e.tick();
    assert!(
        e.host_state
            .logs
            .iter()
            .any(|(id, text)| *id == unknown && text.contains("Node is not executed"))
    );
}

#[test]
fn inferred_stub_pins_use_the_other_ends_direction() {
    let mut e = TickEngine::new();
    let known = fixed(
        &mut e,
        vec![
            pin(7, PinDirection::Input, DataType::String),
            pin(7, PinDirection::Output, DataType::Number),
        ],
        vec![],
    );
    let unknown = NodeId::generate();
    let mut graph = e.export_graph();
    graph.nodes.push(NodeModel {
        id: unknown,
        type_name: "missing/node".into(),
        properties: vec![],
        position: Position::default(),
        pins: None,
        size: None,
    });
    graph.data_connections.push(ConnectionModel {
        src_node: known,
        src_pin: PinId(7),
        dst_node: unknown,
        dst_pin: PinId(1),
    });
    assert!(e.load_graph(&graph).is_empty());
    let def = e.node_definition(unknown).unwrap();
    assert_eq!(def.pins.len(), 1);
    assert_eq!(def.pins[0].pin_type, PinType::Data(DataType::Number));
    e.trigger_node(unknown, PinId(0));
    e.tick();
    assert_eq!(
        e.last_tick,
        TickStats {
            spent: 1,
            deferred: 0
        }
    );
    assert_eq!(e.executed.len(), 1);
    assert_eq!(e.executed[0].inputs, [(PinId(1), None)]);
    assert!(
        e.host_state
            .logs
            .iter()
            .any(|(id, text)| *id == unknown && text.contains("Node is not executed"))
    );
}

#[test]
fn unavailable_bare_nodes_charge_work_and_preserve_saved_properties() {
    let mut e = TickEngine::new();
    let id = NodeId::generate();
    let graph = GraphModel {
        nodes: vec![NodeModel {
            id,
            type_name: "missing/node".into(),
            properties: vec![("value".into(), 5i64.into())],
            position: Position::default(),
            pins: None,
            size: None,
        }],
        exec_connections: vec![],
        data_connections: vec![],
        wire_routes: Vec::new(),
        detached_connections: Vec::new(),
        frames: Vec::new(),
    };
    assert!(e.load_graph(&graph).is_empty());
    e.trigger_node(id, PinId(0));
    e.tick();
    assert_eq!(
        e.last_tick,
        TickStats {
            spent: 1,
            deferred: 0
        }
    );
    let mut expected = graph;
    expected.nodes[0].pins = Some(vec![]);
    assert_eq!(e.export_graph(), expected);
    assert_eq!(
        e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(),
        [id]
    );
    assert!(
        e.host_state
            .logs
            .iter()
            .any(|(owner, text)| *owner == id && text.contains("missing/node"))
    );
}
