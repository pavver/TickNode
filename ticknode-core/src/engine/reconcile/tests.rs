use super::*;

#[test]
fn diff_ignores_positions_but_recreates_changed_properties_and_types() {
    let mut engine = TickEngine::new();
    let ids: Vec<_> = (0..4).map(|_| {
        let id = NodeId::generate();
        engine.create_node(id, "for_loop", &[]).unwrap();
        id
    }).collect();
    engine.connect_exec(ids[0], PinId(4), ids[1], PinId(0)).unwrap();
    engine.connect_data(ids[0], PinId(5), ids[1], PinId(1)).unwrap();
    let current = engine.export_graph();
    let mut target = current.clone();
    target.nodes.iter_mut().find(|n| n.id == ids[0]).unwrap().position.x = 100.0;
    target.nodes.iter_mut().find(|n| n.id == ids[1]).unwrap().properties.push(("in:1".into(), DataValue::from(8.0)));
    target.nodes.iter_mut().find(|n| n.id == ids[2]).unwrap().type_name = "const".into();
    target.nodes.retain(|n| n.id != ids[3]);
    let new = NodeId::generate();
    target.nodes.push(NodeModel { id: new, type_name: "const".into(), properties: vec![], position: Position::default() });
    let plan = diff(&current, &target);
    assert!(!plan.removed_nodes.contains(&ids[0]));
    assert_eq!(plan.removed_nodes.len(), 3);
    for id in &ids[1..] { assert!(plan.removed_nodes.contains(id)); }
    let inserted: Vec<_> = plan.insert.nodes.iter().map(|n| n.id).collect();
    assert_eq!(inserted.len(), 3);
    for id in [ids[1], ids[2], new] { assert!(inserted.contains(&id)); }
    assert_eq!(plan.disconnected.exec, current.exec_connections);
    assert_eq!(plan.disconnected.data, current.data_connections);
    assert_eq!(plan.insert.exec_connections, target.exec_connections);
    assert_eq!(plan.insert.data_connections, target.data_connections);
    assert_eq!(current, engine.export_graph(), "diff must not mutate its inputs");
}

#[test]
fn diff_connections_between_kept_nodes_and_idempotence() {
    let a = NodeId::generate();
    let b = NodeId::generate();
    let c = ConnectionModel { src_node: a, src_pin: PinId(4), dst_node: b, dst_pin: PinId(0) };
    let nodes = [a, b].into_iter().map(|id| NodeModel {
        id, type_name: "for_loop".into(), properties: vec![], position: Position::default(),
    }).collect();
    let current = GraphModel { nodes, exec_connections: vec![c], data_connections: vec![] };
    let target = GraphModel { exec_connections: vec![], data_connections: vec![c], ..current.clone() };
    let plan = diff(&current, &target);
    assert!(plan.removed_nodes.is_empty());
    assert!(plan.insert.nodes.is_empty());
    assert_eq!(plan.disconnected.exec, vec![c]);
    assert_eq!(plan.insert.data_connections, vec![c]);
    let plan = diff(&target, &target);
    assert!(plan.removed_nodes.is_empty());
    assert_eq!(plan.insert, GraphModel::default());
    assert_eq!(plan.disconnected, RemovedConnections::default());
}

#[test]
fn reconcile_invalidates_pure_dependents_when_a_source_is_removed_or_recreated() {
    let mut engine = TickEngine::new();
    let source = NodeId::generate();
    engine.create_node(source, "const", &[]).unwrap();
    engine.set_node_property(source, "value", DataValue::from(5.0)).unwrap();
    let compare = NodeId::generate();
    engine.create_node(compare, "compare", &[]).unwrap();
    engine.connect_data(source, PinId(0), compare, PinId(0)).unwrap();
    assert_eq!(engine.pull_data(compare, PinId(3), &mut vec![]), Some(DataValue::Bool(true)));
    let mut target = engine.export_graph();
    target.nodes.retain(|n| n.id != source);
    target.data_connections.clear();
    assert!(engine.reconcile(&target).warnings.is_empty());
    assert_eq!(engine.pull_data(compare, PinId(3), &mut vec![]), Some(DataValue::Bool(false)));

    let target = engine.export_graph();
    engine.set_input_value(compare, PinId(0), Some(DataValue::from(10.0))).unwrap();
    assert_eq!(engine.pull_data(compare, PinId(3), &mut vec![]), Some(DataValue::Bool(true)));
    assert!(engine.reconcile(&target).warnings.is_empty());
    assert_eq!(engine.pull_data(compare, PinId(3), &mut vec![]), Some(DataValue::Bool(false)));
}
