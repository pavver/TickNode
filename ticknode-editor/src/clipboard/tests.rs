use super::*;

#[test]
fn snippets_roundtrip_positions_properties_and_connections() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let connection = ConnectionModel {
        src_node: a,
        src_pin: PinId(3),
        dst_node: b,
        dst_pin: PinId(4),
    };
    let graph = GraphModel {
        nodes: vec![NodeModel {
            id: a,
            type_name: "constant".into(),
            position: Position { x: 12.0, y: -8.0 },
            properties: vec![("value".into(), DataValue::String("hello".into()))],
            pins: None,
            size: Some(NodeSize {
                width: 384.0,
                height: 240.0,
            }),
        }],
        exec_connections: vec![connection],
        data_connections: vec![connection],
        wire_routes: Vec::new(),
        detached_connections: Vec::new(),
        frames: Vec::new(),
    };
    assert_eq!(
        from_text(&format!(" \n{}\n ", to_text(graph.clone()))),
        Some(graph)
    );
    assert_eq!(from_text("garbage"), None);
    assert_eq!(from_text(&to_text(GraphModel::default())), None);
    let unknown = to_text(GraphModel::default()).replace(FORMAT, "other@1");
    assert_eq!(from_text(&unknown), None);
}

#[test]
fn snapping_uses_the_same_grid_on_both_axes() {
    assert_eq!(snap_point((10.0, -10.0)), (12.0, -12.0));
    assert_eq!(snap_point((8.0, 12.0)), (8.0, 12.0));
}

#[test]
fn snippet_routes_survive_json_and_follow_normalization_and_snapped_placement() {
    let id = NodeId::generate();
    let connection = ConnectionModel {
        src_node: id,
        src_pin: PinId(1),
        dst_node: id,
        dst_pin: PinId(2),
    };
    let mut graph = GraphModel {
        nodes: vec![NodeModel {
            id,
            type_name: "for_loop".into(),
            properties: vec![],
            position: Position { x: 20.0, y: 40.0 },
            pins: None,
            size: None,
        }],
        exec_connections: vec![connection],
        wire_routes: vec![WireRoute {
            exec: true,
            connection,
            points: vec![Position { x: 60.0, y: 80.0 }],
        }],
        detached_connections: Vec::new(),
        ..Default::default()
    };
    assert_eq!(from_text(&to_text(graph.clone())), Some(graph.clone()));
    offset_routes(&mut graph, (-20.0, -40.0), false);
    assert_eq!(graph.wire_routes[0].points, [Position { x: 40.0, y: 40.0 }]);
    offset_routes(&mut graph, (102.0, -50.0), true);
    assert_eq!(
        graph.wire_routes[0].points,
        [Position { x: 144.0, y: -12.0 }]
    );
}

#[test]
fn frame_snippet_survives_json_and_offsets_with_nodes() {
    let frame_id = FrameId::generate();
    let frame = Frame {
        id: frame_id,
        title: "Test Frame".into(),
        color: FrameColor::Yellow,
        position: Position { x: 50.0, y: 100.0 },
        size: NodeSize {
            width: 400.0,
            height: 300.0,
        },
    };
    let node_id = NodeId::generate();
    let node = NodeModel {
        id: node_id,
        type_name: "test".into(),
        properties: vec![],
        position: Position { x: 80.0, y: 140.0 },
        pins: None,
        size: None,
    };
    let graph = GraphModel {
        nodes: vec![node],
        frames: vec![frame],
        ..Default::default()
    };
    assert_eq!(from_text(&to_text(graph.clone())), Some(graph.clone()));

    // Standalone frame without nodes also roundtrips
    let frame_only_graph = GraphModel {
        frames: vec![graph.frames[0].clone()],
        ..Default::default()
    };
    assert_eq!(
        from_text(&to_text(frame_only_graph.clone())),
        Some(frame_only_graph)
    );
}
