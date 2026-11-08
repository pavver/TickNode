use super::*;

#[test]
fn snippets_roundtrip_positions_properties_and_connections() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let connection = ConnectionModel { src_node: a, src_pin: PinId(3), dst_node: b, dst_pin: PinId(4) };
    let graph = GraphModel {
        nodes: vec![NodeModel { id: a, type_name: "constant".into(), position: Position { x: 12.0, y: -8.0 }, properties: vec![("value".into(), DataValue::String("hello".into()))] }],
        exec_connections: vec![connection], data_connections: vec![connection],
    };
    assert_eq!(from_text(&format!(" \n{}\n ", to_text(graph.clone()))), Some(graph));
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
