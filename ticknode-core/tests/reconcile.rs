use ticknode_core::{GraphReconcile, TickEngine};
use ticknode_shared::*;

fn node(engine: &mut TickEngine, kind: &str) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, kind, &[]).unwrap();
    id
}

fn unchanged(result: &GraphReconcile) {
    assert!(result.removed_nodes.is_empty());
    assert!(result.added_nodes.is_empty());
    assert!(result.disconnected.exec.is_empty());
    assert!(result.disconnected.data.is_empty());
    assert!(result.connected_exec.is_empty());
    assert!(result.connected_data.is_empty());
    assert!(result.warnings.is_empty());
}

fn restore(engine: &mut TickEngine, target: &GraphModel) -> GraphReconcile {
    let result = engine.reconcile(target);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(engine.export_graph(), *target);
    unchanged(&engine.reconcile(target));
    result
}

#[test]
fn reconcile_add_remove_and_preserve_runtime() {
    let mut engine = TickEngine::new();
    let kept = node(&mut engine, "for_loop");
    let task = engine.host_state.start_delay(kept, 100);
    engine.trigger_node(kept, PinId(0));
    engine.variables.create(VariableKey::global("survives"), DataType::Bool, DataValue::Bool(true));
    let target = engine.export_graph();
    let added = node(&mut engine, "const");
    let result = restore(&mut engine, &target);
    assert_eq!(result.removed_nodes, vec![added]);
    assert!(result.added_nodes.is_empty());
    assert!(!result.removed_nodes.contains(&kept));
    assert!(engine.host_state.active_tasks.contains_key(&task));
    assert_eq!(engine.execution_queue.front(), Some(&(kept, PinId(0))));
    assert_eq!(engine.variables.get(&VariableKey::global("survives")).unwrap().value, DataValue::Bool(true));

    let other = node(&mut engine, "for_loop");
    engine.connect_exec(kept, PinId(4), other, PinId(0)).unwrap();
    engine.connect_data(kept, PinId(5), other, PinId(1)).unwrap();
    let target = engine.export_graph();
    engine.remove_node(other);
    let result = restore(&mut engine, &target);
    assert_eq!(result.added_nodes, vec![other]);
    assert_eq!(result.connected_exec, target.exec_connections);
    assert_eq!(result.connected_data, target.data_connections);
}

#[test]
fn reconcile_properties_inline_values_and_split_pins() {
    let mut engine = TickEngine::new();
    engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
    ])], &[]).unwrap();
    let constant = node(&mut engine, "const");
    let maker = node(&mut engine, "type/make/test.point");
    engine.set_pin_split(maker, PinId(0), true).unwrap();
    engine.set_input_value(maker, PinId(1), Some(DataValue::from(7.0))).unwrap();
    let target = engine.export_graph();
    engine.set_node_property(constant, "value", DataValue::from(42.0)).unwrap();
    engine.set_input_value(maker, PinId(1), Some(DataValue::from(9.0))).unwrap();
    engine.set_pin_split(maker, PinId(0), false).unwrap();
    let result = restore(&mut engine, &target);
    for id in [constant, maker] {
        assert!(result.removed_nodes.contains(&id));
        assert!(result.added_nodes.contains(&id));
    }
}

#[test]
fn reconcile_connections_without_recreating_nodes() {
    let mut engine = TickEngine::new();
    let a = node(&mut engine, "for_loop");
    let b = node(&mut engine, "for_loop");
    let empty = engine.export_graph();
    engine.connect_exec(a, PinId(4), b, PinId(0)).unwrap();
    engine.connect_data(a, PinId(5), b, PinId(1)).unwrap();
    let connected = engine.export_graph();
    let result = restore(&mut engine, &empty);
    assert!(result.removed_nodes.is_empty());
    assert!(result.added_nodes.is_empty());
    assert_eq!(result.disconnected.exec, connected.exec_connections);
    assert_eq!(result.disconnected.data, connected.data_connections);
    let result = restore(&mut engine, &connected);
    assert!(result.removed_nodes.is_empty());
    assert!(result.added_nodes.is_empty());
    assert_eq!(result.connected_exec, connected.exec_connections);
    assert_eq!(result.connected_data, connected.data_connections);
    let result = restore(&mut engine, &GraphModel::default());
    assert_eq!(result.disconnected.exec, connected.exec_connections);
    assert_eq!(result.disconnected.data, connected.data_connections);
}

#[test]
fn reconcile_recreates_types_and_unknown_types_as_stubs() {
    let mut engine = TickEngine::new();
    let source = node(&mut engine, "for_loop");
    let sink = node(&mut engine, "for_loop");
    engine.connect_exec(source, PinId(4), sink, PinId(0)).unwrap();
    engine.connect_data(source, PinId(5), sink, PinId(1)).unwrap();
    let mut target = engine.export_graph();
    target.nodes.iter_mut().find(|n| n.id == source).unwrap().type_name = "missing/plugin".into();
    let result = restore(&mut engine, &target);
    assert_eq!(result.removed_nodes, vec![source]);
    assert_eq!(result.added_nodes, vec![source]);
    assert!(engine.node_unavailable(source).is_some());
    assert_eq!(result.disconnected.exec, target.exec_connections);
    assert_eq!(result.disconnected.data, target.data_connections);
}
