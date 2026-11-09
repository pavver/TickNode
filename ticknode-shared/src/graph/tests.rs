use super::*;

#[test]
fn legacy_json_defaults_to_automatic_size_and_postcard_keeps_size_fields() {
    let id = NodeId::generate();
    let json = serde_json::json!({"id": id, "type_name": "const", "properties": [], "position": {"x": 0, "y": 0}});
    let mut node: NodeModel = serde_json::from_value(json).unwrap();
    assert_eq!(node.size, None);
    for size in [None, Some(NodeSize { width: 320.0, height: 240.0 })] {
        node.size = size;
        // Multiple nodes exercise field alignment even when the optional size is absent.
        let graph = GraphModel { nodes: vec![node.clone(), node.clone()], ..Default::default() };
        let bytes = postcard::to_allocvec(&graph).unwrap();
        assert_eq!(postcard::from_bytes::<GraphModel>(&bytes).unwrap(), graph);
        assert_eq!(serde_json::from_str::<GraphModel>(&serde_json::to_string(&graph).unwrap()).unwrap(), graph);
    }
}

#[test]
fn legacy_json_defaults_to_empty_frames_and_postcard_roundtrip_keeps_frames() {
    let json = serde_json::json!({
        "nodes": [],
        "exec_connections": [],
        "data_connections": [],
    });
    let graph: GraphModel = serde_json::from_value(json).unwrap();
    assert!(graph.frames.is_empty());

    let frame = Frame {
        id: FrameId::generate(),
        title: "Test Frame".into(),
        color: FrameColor::Teal,
        position: Position { x: 10.0, y: 20.0 },
        size: NodeSize { width: 300.0, height: 200.0 },
    };
    let graph = GraphModel {
        frames: vec![frame],
        ..Default::default()
    };
    let bytes = postcard::to_allocvec(&graph).unwrap();
    assert_eq!(postcard::from_bytes::<GraphModel>(&bytes).unwrap(), graph);
    assert_eq!(serde_json::from_str::<GraphModel>(&serde_json::to_string(&graph).unwrap()).unwrap(), graph);
}

#[test]
fn frame_validation_and_clamping() {
    let id = FrameId::generate();
    let valid = Frame {
        id,
        title: "Valid Frame".into(),
        color: FrameColor::Green,
        position: Position { x: 50.0, y: 60.0 },
        size: NodeSize { width: 200.0, height: 150.0 },
    };
    assert_eq!(valid.clone().validated(), Some(valid.clone()));

    // Non-finite position or size is rejected
    assert_eq!(Frame { position: Position { x: f32::NAN, y: 0.0 }, ..valid.clone() }.validated(), None);
    assert_eq!(Frame { position: Position { x: 0.0, y: f32::INFINITY }, ..valid.clone() }.validated(), None);
    assert_eq!(Frame { size: NodeSize { width: f64::NAN, height: 100.0 }, ..valid.clone() }.validated(), None);
    assert_eq!(Frame { size: NodeSize { width: 200.0, height: f64::INFINITY }, ..valid.clone() }.validated(), None);

    // Clamping size
    let clamped = Frame {
        size: NodeSize { width: 10.0, height: 9999.0 },
        ..valid.clone()
    }.validated().unwrap();
    assert_eq!(clamped.size, NodeSize { width: MIN_FRAME_W, height: MAX_FRAME_SIZE });

    // Truncating title at MAX_FRAME_TITLE chars safely (including multi-byte UTF-8)
    let long_title = "ї".repeat(MAX_FRAME_TITLE + 50);
    let truncated = Frame {
        title: long_title,
        ..valid.clone()
    }.validated().unwrap();
    assert_eq!(truncated.title.chars().count(), MAX_FRAME_TITLE);
    assert_eq!(truncated.title, "ї".repeat(MAX_FRAME_TITLE));
}
