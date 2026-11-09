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
