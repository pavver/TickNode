use crate::*;
use ticknode_shared::*;

fn add(e: &mut TickEngine, name: &str) -> NodeId {
    let id = NodeId::generate();
    e.create_node(id, name, &[]).unwrap();
    id
}

fn pin(id: u32, direction: PinDirection, ty: DataType) -> PinDefinition {
    PinDefinition { id: PinId(id), name: format!("Value {id}"), direction, pin_type: PinType::Data(ty), required: false }
}

// A native fixture with deliberately overlapping input/output ids, as supported by the graph model.
struct FixedNode { id: NodeId, pins: Vec<PinDefinition>, outputs: Vec<NodeOutput> }
impl Node for FixedNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 1, type_name: "test/fixed".into(), title: "Fixed".into(), category: "Test".into(),
            description: String::new(), help: String::new(), properties: vec![], pins: self.pins.clone(),
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> { self.outputs.clone() }
}

fn fixed(e: &mut TickEngine, pins: Vec<PinDefinition>, outputs: Vec<NodeOutput>) -> NodeId {
    e.registry.register(Box::new(move |id| Box::new(FixedNode { id, pins: pins.clone(), outputs: outputs.clone() })));
    add(e, "test/fixed")
}

fn point_type(e: &mut TickEngine) {
    e.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
    ])], &[]).unwrap();
}

fn point(x: i64) -> DataValue {
    DataValue::Object {
        type_name: Some("test.point".into()),
        fields: [("x".into(), ObjectField { name: "x".into(), description: String::new(), value: x.into() })].into(),
    }
}

#[test]
fn one_unit_of_prior_work_defers_an_oversized_activation() {
    let mut e = TickEngine::new();
    e.budget.limit = Some(1);
    let button = add(&mut e, "test/click_button");
    let expensive = add(&mut e, "list/add");
    e.trigger_node(button, PinId(0));
    e.trigger_node(expensive, PinId(0));
    e.tick();
    assert_eq!(e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(), [button]);
    assert_eq!(e.last_tick, TickStats { spent: 1, deferred: 1 });
    e.tick();
    assert_eq!(e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(), [expensive]);
    assert_eq!(e.last_tick, TickStats { spent: 5, deferred: 0 });
}

#[test]
fn generated_make_and_break_charge_for_pulled_work() {
    let mut e = TickEngine::new();
    point_type(&mut e);
    let make = add(&mut e, "type/make/test.point");
    let split = add(&mut e, "type/break/test.point");
    let console = add(&mut e, "console");
    e.set_input_value(make, PinId(1), Some(17i64.into())).unwrap();
    e.connect_data(make, PinId(0), split, PinId(0)).unwrap();
    e.connect_data(split, PinId(1), console, PinId(1)).unwrap();
    e.trigger_node(console, PinId(0));
    e.tick();
    assert_eq!(e.host_state.logs, [(console, "17".into())]);
    assert_eq!(e.last_tick, TickStats { spent: 3, deferred: 0 });
    assert_eq!(e.executed.iter().find(|t| t.node_id == make).unwrap().outputs, [(PinId(0), point(17))]);
}

#[test]
fn data_cycles_are_rejected_through_a_shared_dependency() {
    let mut e = TickEngine::new();
    let a = add(&mut e, "math/add");
    let b = add(&mut e, "math/add");
    let c = add(&mut e, "math/add");
    e.set_input_value(a, PinId(0), Some(3i64.into())).unwrap();
    e.connect_data(a, PinId(2), b, PinId(0)).unwrap();
    e.connect_data(a, PinId(2), c, PinId(0)).unwrap();
    e.connect_data(b, PinId(2), c, PinId(1)).unwrap();
    let before = e.export_graph();
    assert!(matches!(e.connect_data(c, PinId(2), a, PinId(1)), Err(EngineError::CycleDetected(..))));
    assert_eq!(e.export_graph(), before);
    assert_eq!(e.pull_data(c, PinId(2), &mut vec![]), Some(6i64.into()));
}

#[test]
fn exec_only_outputs_preserve_pure_work_for_the_tick_budget() {
    let mut e = TickEngine::new();
    let value = add(&mut e, "const");
    let console = add(&mut e, "console");
    let button = add(&mut e, "test/click_button");
    e.connect_data(value, PinId(0), console, PinId(1)).unwrap();
    e.trigger_node(console, PinId(0));
    e.trigger_node(button, PinId(0));
    e.trigger_node(console, PinId(0));
    e.tick();
    assert_eq!(e.host_state.logs, [(console, "0".into()), (console, "0".into())]);
    assert_eq!(e.last_tick.spent, 4, "an exec pulse and an empty output list do not invalidate pure results");
}

#[test]
fn variable_only_outputs_refresh_pure_reads_in_the_same_tick() {
    let mut e = TickEngine::new();
    let read = add(&mut e, "variable/get");
    let console = add(&mut e, "console");
    // Deliberately emit no data: a variable write alone must invalidate a prior read.
    let write = fixed(&mut e, vec![], vec![NodeOutput::Variable("my_var".into(), 23i64.into())]);
    e.connect_data(read, PinId(0), console, PinId(1)).unwrap();
    for id in [console, write, console] { e.trigger_node(id, PinId(0)); }
    e.tick();
    assert_eq!(e.variables["my_var"], 23i64.into());
    assert_eq!(e.host_state.logs, [(console, "0".into()), (console, "23".into())]);
}

#[test]
fn data_only_outputs_refresh_pure_dependents_in_the_same_tick() {
    let mut e = TickEngine::new();
    let source = fixed(&mut e, vec![pin(7, PinDirection::Output, DataType::Number)], vec![NodeOutput::Data(PinId(7), 11i64.into())]);
    let sum = add(&mut e, "math/add");
    let console = add(&mut e, "console");
    e.connect_data(source, PinId(7), sum, PinId(0)).unwrap();
    e.connect_data(sum, PinId(2), console, PinId(1)).unwrap();
    for id in [console, source, console] { e.trigger_node(id, PinId(0)); }
    e.tick();
    assert_eq!(e.data_cache[&source][&PinId(7)], 11i64.into());
    assert_eq!(e.host_state.logs, [(console, "0".into()), (console, "11".into())]);
}

#[test]
fn split_traces_project_outputs_and_hide_the_parent_even_with_shared_ids() {
    let mut e = TickEngine::new();
    point_type(&mut e);
    let source = fixed(&mut e, vec![
        pin(0, PinDirection::Input, DataType::Number),
        pin(0, PinDirection::Output, DataType::Named("test.point".into())),
        pin(3, PinDirection::Output, DataType::Number),
    ], vec![NodeOutput::Data(PinId(0), point(29)), NodeOutput::Data(PinId(3), 99i64.into())]);
    e.set_input_value(source, PinId(0), Some(5i64.into())).unwrap();
    e.set_pin_split(source, PinId(0), true).unwrap();
    let field = split_field_pin(PinId(0), 0).unwrap();
    let console = add(&mut e, "console");
    e.connect_data(source, field, console, PinId(1)).unwrap();
    e.trigger_node(source, PinId(0));
    e.trigger_node(console, PinId(0));
    e.tick();
    assert_eq!(e.host_state.logs, [(console, "29".into())]);
    let trace = e.executed.iter().find(|t| t.node_id == source).unwrap();
    assert_eq!(trace.inputs, [(PinId(0), Some(5i64.into()))]);
    assert_eq!(trace.outputs, [(PinId(3), 99i64.into()), (field, 29i64.into())]);
}

#[test]
fn conversions_use_the_output_type_when_input_and_output_ids_overlap() {
    let mut e = TickEngine::new();
    let source = fixed(&mut e, vec![
        pin(0, PinDirection::Input, DataType::Number),
        pin(0, PinDirection::Output, DataType::String),
    ], vec![NodeOutput::Data(PinId(0), "31".into())]);
    let write = add(&mut e, "variable/set");
    e.connect_data(source, PinId(0), write, PinId(1)).unwrap();
    e.trigger_node(source, PinId(0));
    e.trigger_node(write, PinId(0));
    e.tick();
    assert_eq!(e.variables["my_var"], 31i64.into());
    assert_eq!(e.executed.iter().find(|t| t.node_id == write).unwrap().inputs, [(PinId(1), Some(31i64.into()))]);
}

#[test]
fn remove_node_removes_only_incident_exec_and_data_connections() {
    let mut e = TickEngine::new();
    let ids: Vec<_> = (0..4).map(|_| add(&mut e, "variable/set")).collect();
    let values: Vec<_> = (0..3).map(|_| add(&mut e, "const")).collect();
    for (src, dst) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[3])] {
        e.connect_exec(src, PinId(0), dst, PinId(0)).unwrap();
    }
    for (src, dst) in [(values[0], ids[0]), (values[1], ids[1]), (values[2], ids[2])] {
        e.connect_data(src, PinId(0), dst, PinId(1)).unwrap();
    }
    // Remove a middle exec node/data destination, then a data source.
    let removed = e.remove_node(ids[1]);
    assert_eq!(removed.exec.len(), 2);
    assert_eq!(removed.data.len(), 1);
    assert_eq!(e.exec_connections(), [ConnectionModel { src_node: ids[2], src_pin: PinId(0), dst_node: ids[3], dst_pin: PinId(0) }]);
    assert_eq!(e.data_connections().len(), 2);
    let removed = e.remove_node(values[0]);
    assert_eq!(removed.data, [ConnectionModel { src_node: values[0], src_pin: PinId(0), dst_node: ids[0], dst_pin: PinId(1) }]);
    assert_eq!(e.data_connections(), [ConnectionModel { src_node: values[2], src_pin: PinId(0), dst_node: ids[2], dst_pin: PinId(1) }]);
    e.trigger_node(ids[2], PinId(0));
    e.tick();
    e.tick();
    assert_eq!(e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(), [ids[3]]);
    assert_eq!(e.variables["my_var"], 0i64.into());
}

#[test]
fn changing_a_source_type_prunes_incompatible_outgoing_connections() {
    let mut e = TickEngine::new();
    let source = add(&mut e, "const");
    let sum = add(&mut e, "math/add");
    e.set_input_value(sum, PinId(0), Some(7i64.into())).unwrap();
    e.connect_data(source, PinId(0), sum, PinId(0)).unwrap();
    let removed = e.set_node_property(source, "value", DataValue::Object { fields: Default::default(), type_name: None }).unwrap();
    assert_eq!(removed.data, [ConnectionModel { src_node: source, src_pin: PinId(0), dst_node: sum, dst_pin: PinId(0) }]);
    assert!(e.data_connections().is_empty());
    assert_eq!(e.pull_data(sum, PinId(2), &mut vec![]), Some(7i64.into()));
}

#[test]
fn default_constant_supplies_zero_without_properties() {
    let mut e = TickEngine::new();
    let source = add(&mut e, "const");
    let console = add(&mut e, "console");
    e.connect_data(source, PinId(0), console, PinId(1)).unwrap();
    e.trigger_node(console, PinId(0));
    e.tick();
    assert_eq!(e.host_state.logs, [(console, "0".into())]);
    assert_eq!(e.executed.iter().find(|t| t.node_id == source).unwrap().outputs, [(PinId(0), 0i64.into())]);
}

#[test]
fn a_new_loop_ignores_next_and_break_until_started() {
    let mut e = TickEngine::new();
    let loop_id = add(&mut e, "for_loop");
    let console = add(&mut e, "console");
    e.connect_exec(loop_id, PinId(6), console, PinId(0)).unwrap();
    for pin in [3, 2] { e.trigger_node(loop_id, PinId(pin)); }
    e.tick();
    assert_eq!(e.executed.len(), 1);
    assert_eq!(e.executed[0].node_id, loop_id);
    assert!(e.executed[0].fired.is_empty());
    e.tick();
    assert!(e.host_state.logs.is_empty());
    e.set_input_value(loop_id, PinId(1), Some(2i64.into())).unwrap();
    e.trigger_node(loop_id, PinId(0));
    e.tick();
    assert_eq!(e.data_cache[&loop_id][&PinId(5)], 0i64.into());
    assert_eq!(e.executed[0].fired, [PinId(4)]);
}

#[test]
fn splitting_one_input_preserves_another_unsplit_struct_input() {
    let mut e = TickEngine::new();
    point_type(&mut e);
    let source = add(&mut e, "type/make/test.point");
    e.set_input_value(source, PinId(1), Some(41i64.into())).unwrap();
    let sink = fixed(&mut e, vec![
        pin(0, PinDirection::Input, DataType::Named("test.point".into())),
        pin(1, PinDirection::Input, DataType::Named("test.point".into())),
    ], vec![NodeOutput::Variable("seen".into(), true.into())]);
    e.connect_data(source, PinId(0), sink, PinId(0)).unwrap();
    e.set_pin_split(sink, PinId(1), true).unwrap();
    let field = split_field_pin(PinId(1), 0).unwrap();
    e.set_input_value(sink, field, Some(43i64.into())).unwrap();
    e.trigger_node(sink, PinId(0));
    e.tick();
    let trace = e.executed.iter().find(|t| t.node_id == sink).unwrap();
    assert_eq!(trace.inputs, [(PinId(0), Some(point(41))), (PinId(1), Some(point(43))), (field, Some(43i64.into()))]);
    assert_eq!(e.variables["seen"], true.into());
}

#[test]
fn unknown_graph_nodes_keep_inferred_pin_types_order_and_fanout() {
    let mut e = TickEngine::new();
    let pins = vec![
        pin(7, PinDirection::Input, DataType::String),
        pin(7, PinDirection::Output, DataType::Number),
        pin(8, PinDirection::Input, DataType::Bool),
    ];
    let a = fixed(&mut e, pins.clone(), vec![NodeOutput::Data(PinId(7), 13i64.into())]);
    let b = fixed(&mut e, pins, vec![NodeOutput::Data(PinId(7), 13i64.into())]);
    let button = add(&mut e, "test/click_button");
    let console = add(&mut e, "console");
    let unknown = NodeId::generate();
    let mut graph = e.export_graph();
    graph.nodes.push(NodeModel { id: unknown, type_name: "missing/node".into(), properties: vec![("saved".into(), "kept".into())], position: Position::default() });
    let connection = |src_node, src_pin, dst_node, dst_pin| ConnectionModel { src_node, src_pin: PinId(src_pin), dst_node, dst_pin: PinId(dst_pin) };
    graph.exec_connections = vec![connection(button, 0, unknown, 0), connection(unknown, 0, console, 0)];
    graph.data_connections = vec![
        connection(a, 7, unknown, 3), connection(unknown, 4, a, 7),
        connection(unknown, 4, b, 7), connection(unknown, 6, b, 8),
    ];
    // The feedback through a would form a data cycle; use a separate source for the incoming edge.
    let source = add(&mut e, "const");
    graph.nodes.push(e.node_model(source).unwrap());
    graph.data_connections[0].src_node = source;
    graph.data_connections[0].src_pin = PinId(0);
    assert!(e.load_graph(&graph).is_empty());
    let def = e.node_definition(unknown).unwrap();
    let signature: Vec<_> = def.pins.iter().map(|p| (p.id, p.direction, p.pin_type.clone(), p.required)).collect();
    assert_eq!(signature, [
        (PinId(0), PinDirection::Input, PinType::Exec, false),
        (PinId(3), PinDirection::Input, PinType::Data(DataType::Number), false),
        (PinId(0), PinDirection::Output, PinType::Exec, false),
        (PinId(4), PinDirection::Output, PinType::Data(DataType::String), false),
        (PinId(6), PinDirection::Output, PinType::Data(DataType::Bool), false),
    ]);
    assert_eq!(e.exec_connections().len(), 2);
    assert_eq!(e.data_connections().len(), 4);
    assert_eq!(e.node_model(unknown).unwrap().properties, [("saved".into(), "kept".into())]);
    // Triggering a stub answers with its unavailable error, even with absent connected data.
    e.trigger_node(unknown, PinId(0));
    e.tick();
    assert!(e.host_state.logs.iter().any(|(id, text)| *id == unknown && text.contains("Node is not executed")));
}

#[test]
fn inferred_stub_pins_use_the_other_ends_direction() {
    let mut e = TickEngine::new();
    let known = fixed(&mut e, vec![
        pin(7, PinDirection::Input, DataType::String),
        pin(7, PinDirection::Output, DataType::Number),
    ], vec![]);
    let unknown = NodeId::generate();
    let mut graph = e.export_graph();
    graph.nodes.push(NodeModel { id: unknown, type_name: "missing/node".into(), properties: vec![], position: Position::default() });
    graph.data_connections.push(ConnectionModel { src_node: known, src_pin: PinId(7), dst_node: unknown, dst_pin: PinId(1) });
    assert!(e.load_graph(&graph).is_empty());
    let def = e.node_definition(unknown).unwrap();
    assert_eq!(def.pins.len(), 1);
    assert_eq!(def.pins[0].pin_type, PinType::Data(DataType::Number));
    e.trigger_node(unknown, PinId(0));
    e.tick();
    assert_eq!(e.last_tick, TickStats { spent: 1, deferred: 0 });
    assert_eq!(e.executed.len(), 1);
    assert_eq!(e.executed[0].inputs, [(PinId(1), None)]);
    assert!(e.host_state.logs.iter().any(|(id, text)| *id == unknown && text.contains("Node is not executed")));
}

#[test]
fn unavailable_bare_nodes_charge_work_and_preserve_saved_properties() {
    let mut e = TickEngine::new();
    let id = NodeId::generate();
    let graph = GraphModel {
        nodes: vec![NodeModel { id, type_name: "missing/node".into(), properties: vec![("value".into(), 5i64.into())], position: Position::default() }],
        exec_connections: vec![], data_connections: vec![],
    };
    assert!(e.load_graph(&graph).is_empty());
    e.trigger_node(id, PinId(0));
    e.tick();
    assert_eq!(e.last_tick, TickStats { spent: 1, deferred: 0 });
    assert_eq!(e.export_graph(), graph);
    assert_eq!(e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(), [id]);
    assert!(e.host_state.logs.iter().any(|(owner, text)| *owner == id && text.contains("missing/node")));
}
