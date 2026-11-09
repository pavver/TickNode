use super::*;
use crate::{HostState, Node, NodeContext, TaskId, TaskResult};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

#[derive(Default)]
struct Calls { execute: AtomicUsize, poll: AtomicUsize, task: AtomicUsize }

struct Probe { id: NodeId, definition: NodeDefinition, calls: Arc<Calls> }
impl Node for Probe {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition { self.definition.clone() }
    fn is_pure(&self) -> bool { true }
    fn polls(&self) -> bool { true }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        self.calls.execute.fetch_add(1, Ordering::SeqCst);
        vec![NodeOutput::Data(PinId(7), 7i64.into())]
    }
    fn poll(&mut self, _: &mut HostState) -> Option<PinId> {
        self.calls.poll.fetch_add(1, Ordering::SeqCst);
        None
    }
    fn on_task(&mut self, _: TaskId, _: TaskResult, _: &mut NodeContext) -> Vec<NodeOutput> {
        self.calls.task.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
}

fn pin(id: u32, direction: PinDirection, pin_type: PinType) -> PinDefinition {
    PinDefinition { id: PinId(id), name: format!("Pin {id}"), direction, pin_type, required: false }
}
fn definition(name: &str) -> NodeDefinition {
    NodeDefinition {
        type_name: name.into(), title: "Probe".into(), category: "Test".into(), description: String::new(),
        help: String::new(), properties: Vec::new(), cost: 1,
        pins: vec![pin(0, PinDirection::Input, PinType::Exec),
            pin(1, PinDirection::Input, PinType::Data(DataType::Number)),
            pin(7, PinDirection::Output, PinType::Data(DataType::Number))],
    }
}
fn register(e: &mut TickEngine, definition: NodeDefinition, calls: &Arc<Calls>) {
    let calls = Arc::clone(calls);
    e.registry.register(Box::new(move |id| Box::new(Probe { id, definition: definition.clone(), calls: calls.clone() })));
}
fn fixture() -> (TickEngine, GraphModel, ConnectionModel, Arc<Calls>) {
    let mut e = TickEngine::new();
    let calls = Arc::new(Calls::default());
    register(&mut e, definition("probe/source"), &Arc::new(Calls::default()));
    register(&mut e, definition("probe/sink"), &calls);
    let src = NodeId::generate();
    let dst = NodeId::generate();
    e.create_node(src, "probe/source", &[]).unwrap();
    e.create_node(dst, "probe/sink", &[]).unwrap();
    e.connect_data(src, PinId(7), dst, PinId(1)).unwrap();
    let c = e.data_connections()[0];
    let graph = e.export_graph();
    (e, graph, c, calls)
}
fn change_input(e: &mut TickEngine, calls: &Arc<Calls>) {
    let mut def = definition("probe/sink");
    def.pins[1].pin_type = PinType::Data(DataType::Object);
    register(e, def, calls);
}

#[test]
fn legacy_graph_records_signatures_without_freezing() {
    let (mut e, mut graph, _, _) = fixture();
    for n in &mut graph.nodes { n.pins = None; }
    assert!(e.load_graph(&graph).is_empty());
    assert!(e.export_graph().nodes.iter().all(|n| n.pins.is_some() && e.frozen_changes(n.id).is_empty()));
}

#[test]
fn changed_input_detaches_and_blocks_every_execution_path_until_confirmed() {
    let (mut e, graph, c, calls) = fixture();
    change_input(&mut e, &calls);
    assert!(e.load_graph(&graph).is_empty());
    let d = DetachedConnection { exec: false, connection: c, end: DetachedEnd::Target, row: 1 };
    assert_eq!(e.detached_connections(), &[d]);
    assert!(!e.frozen_changes(c.dst_node).is_empty());
    e.host_state.start_delay(c.dst_node, 1);
    for _ in 0..2 {
        e.trigger_node(c.dst_node, PinId(0));
        e.tick();
        assert_eq!(e.pull_data(c.dst_node, PinId(7), &mut vec![]), None);
    }
    assert_eq!(calls.execute.load(Ordering::SeqCst), 0);
    assert_eq!(calls.poll.load(Ordering::SeqCst), 0);
    assert_eq!(calls.task.load(Ordering::SeqCst), 0);
    assert_eq!(e.host_state.logs.iter().filter(|(id, s)| *id == c.dst_node && s.contains("Node is frozen:")).count(), 1);
    let frozen = e.export_graph();
    assert!(e.load_graph(&frozen).is_empty());
    assert_eq!(e.export_graph(), frozen);
    assert!(!e.frozen_changes(c.dst_node).is_empty());
    assert!(e.confirm_pins(c.dst_node).unwrap());
    assert!(!e.confirm_pins(c.dst_node).unwrap());
    assert_eq!(e.detached_connections(), &[d]);
    e.trigger_node(c.dst_node, PinId(0));
    e.tick();
    assert_eq!(calls.execute.load(Ordering::SeqCst), 1);
    assert_eq!(calls.poll.load(Ordering::SeqCst), 1);
    assert_eq!(e.pull_data(c.dst_node, PinId(7), &mut vec![]), Some(7i64.into()));
    assert!(e.remove_detached(&d));
    assert!(!e.remove_detached(&d));
}

#[test]
fn removed_source_and_both_changed_prefer_the_expected_end() {
    let (mut e, graph, c, calls) = fixture();
    let mut def = definition("probe/source");
    def.pins.pop();
    register(&mut e, def, &Arc::new(Calls::default()));
    assert!(e.load_graph(&graph).is_empty());
    assert_eq!(e.detached_connections()[0].end, DetachedEnd::Source);
    assert_eq!(e.detached_connections()[0].row, 0);
    assert!(matches!(e.frozen_changes(c.src_node), [PinChange::Removed { .. }]));
    change_input(&mut e, &calls);
    assert!(e.load_graph(&graph).is_empty());
    assert_eq!(e.detached_connections()[0].end, DetachedEnd::Target);
    assert_eq!(e.detached_connections()[0].row, 1);
}

#[test]
fn additions_optional_and_required_and_renames() {
    for required in [false, true] {
        let (mut e, graph, c, calls) = fixture();
        let mut def = definition("probe/sink");
        def.pins[1].name = "Renamed".into();
        let mut added = pin(5, PinDirection::Input, PinType::Data(DataType::Bool));
        added.required = required;
        def.pins.push(added);
        register(&mut e, def.clone(), &calls);
        assert!(e.load_graph(&graph).is_empty());
        assert_eq!(!e.frozen_changes(c.dst_node).is_empty(), required);
        if !required { assert_eq!(e.node_model(c.dst_node).unwrap().pins, Some(pin_signature(&def))); }
        assert_eq!(e.data_connections(), &[c]);
    }
}

#[test]
fn stubs_retain_confirmed_signatures_and_detach_when_revived() {
    let (mut e, graph, c, calls) = fixture();
    e.replace_with_stubs(&["probe/sink".into()].into(), "probe");
    assert_eq!(e.export_graph(), graph);
    assert!(!e.confirm_pins(c.dst_node).unwrap());
    change_input(&mut e, &calls);
    let (revived, removed, detached) = e.revive_stubs();
    assert_eq!(revived, vec![c.dst_node]);
    assert_eq!(removed, RemovedConnections::default());
    assert_eq!(detached.len(), 1);
    assert_eq!(e.detached_connections(), detached);
    assert!(!e.frozen_changes(c.dst_node).is_empty());
    let frozen = e.export_graph();
    e.replace_with_stubs(&["probe/sink".into()].into(), "probe");
    assert_eq!(e.export_graph(), frozen);
    e.revive_stubs();
    assert_eq!(e.export_graph(), frozen);
}

#[test]
fn user_property_changes_accept_pins_and_history_restores_confirmed_contract() {
    let mut e = TickEngine::new();
    let id = NodeId::generate();
    e.create_node(id, "math/expression", &[("expression".into(), "a + b".into())]).unwrap();
    let before = e.export_graph();
    e.set_node_property(id, "expression", "a".into()).unwrap();
    assert!(e.frozen_changes(id).is_empty());
    assert_eq!(e.node_model(id).unwrap().pins, Some(pin_signature(&e.nodes[&id].definition())));
    assert!(e.reconcile(&before).warnings.is_empty());
    assert_eq!(e.export_graph(), before);

    let (mut e, graph, c, calls) = fixture();
    change_input(&mut e, &calls);
    e.load_graph(&graph);
    let frozen = e.export_graph();
    let task = e.host_state.start_delay(c.dst_node, 100);
    e.required_reported.insert(c.src_node);
    e.confirm_pins(c.dst_node).unwrap();
    let confirmed = e.export_graph();
    assert!(e.reconcile(&frozen).warnings.is_empty());
    assert!(!e.frozen_changes(c.dst_node).is_empty());
    assert_eq!(e.export_graph(), frozen);
    assert!(e.host_state.active_tasks.contains_key(&task));
    assert!(e.required_reported.contains(&c.src_node));
    e.reconcile(&confirmed);
    assert!(e.frozen_changes(c.dst_node).is_empty());
    assert_eq!(e.export_graph(), confirmed);
}

#[test]
fn snippets_remap_detached_and_removing_nodes_clears_all_metadata() {
    let (mut e, graph, c, calls) = fixture();
    change_input(&mut e, &calls);
    e.load_graph(&graph);
    assert!(e.export_snippet(&[c.src_node]).detached_connections.is_empty());
    let snippet = e.export_graph();
    let (ids, warnings) = e.import_snippet(&snippet);
    assert!(warnings.is_empty());
    let mapped = |id| ids.iter().find(|(old, _)| *old == id).unwrap().1;
    let imported = e.export_snippet(&[mapped(c.src_node), mapped(c.dst_node)]);
    assert_eq!(imported.detached_connections[0].connection.src_node, mapped(c.src_node));
    assert_eq!(imported.detached_connections[0].connection.dst_node, mapped(c.dst_node));
    assert!(!e.frozen_changes(mapped(c.dst_node)).is_empty());
    e.remove_node(c.dst_node);
    assert!(!e.confirmed_pins.contains_key(&c.dst_node));
    assert!(e.frozen_changes(c.dst_node).is_empty());
    assert_eq!(e.detached_connections().len(), 1);
    let mut missing = snippet;
    missing.nodes.retain(|n| n.id != c.dst_node);
    e.load_graph(&missing);
    assert!(e.detached_connections().is_empty());
}

#[test]
fn editing_a_frozen_node_keeps_it_frozen_until_confirmed() {
    let (mut e, graph, c, calls) = fixture();
    change_input(&mut e, &calls);
    assert!(e.load_graph(&graph).is_empty());
    let changes = e.frozen_changes(c.dst_node).to_vec();
    e.set_node_property(c.dst_node, "anything", DataValue::Bool(true)).unwrap();
    assert_eq!(e.frozen_changes(c.dst_node), changes.as_slice());
    let mut target = e.export_graph();
    target.nodes.iter_mut().find(|n| n.id == c.dst_node).unwrap().properties.push(("other".into(), DataValue::Bool(false)));
    e.reconcile_settings(&target, c.dst_node);
    assert_eq!(e.frozen_changes(c.dst_node), changes.as_slice());
    assert!(e.confirm_pins(c.dst_node).unwrap());
}
