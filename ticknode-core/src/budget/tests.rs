use crate::*;
use ticknode_shared::*;

struct Probe { id: NodeId, cost: u32, pure: bool, polls: bool }
impl Node for Probe {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        let mut def = ConstNode { id: self.id, value: 1i64.into() }.definition();
        def.type_name = "probe".into();
        def.cost = self.cost;
        def
    }
    fn is_pure(&self) -> bool { self.pure }
    fn polls(&self) -> bool { self.polls }
    fn poll(&mut self, _: &mut HostState) -> Option<PinId> { None }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), 1i64.into())]
    }
}

fn probe(engine: &mut TickEngine, cost: u32, pure: bool, polls: bool) -> NodeId {
    let id = NodeId::generate();
    engine.nodes.insert(id, Box::new(Probe { id, cost, pure, polls }));
    id
}
fn queued(engine: &mut TickEngine, count: usize) -> Vec<NodeId> {
    (0..count).map(|_| {
        let id = probe(engine, 1, false, false);
        engine.trigger_node(id, PinId(0));
        id
    }).collect()
}
fn ran(engine: &TickEngine) -> Vec<NodeId> {
    engine.executed.iter().map(|t| t.node_id).collect()
}

#[test]
fn budget_limits_burst_and_preserves_fifo_ahead_of_new_hops() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(2);
    let ids = queued(&mut engine, 5);
    let child = probe(&mut engine, 1, false, false);
    // A real exec source creates next-tick work while the original burst is draining.
    engine.nodes.insert(ids[0], Box::new(ClickButtonNode { id: ids[0], label: String::new(), clicked: false }));
    engine.connections.add_exec(ConnectionModel { src_node: ids[0], src_pin: PinId(0), dst_node: child, dst_pin: PinId(0) });
    engine.tick();
    assert_eq!(ran(&engine), ids[..2]);
    assert_eq!(engine.last_tick, TickStats { spent: 2, deferred: 3 });
    assert_eq!(engine.execution_queue.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [ids[2], ids[3], ids[4], child]);
    let late = queued(&mut engine, 1)[0];
    engine.tick();
    assert_eq!(ran(&engine), ids[2..4]);
    assert_eq!(engine.last_tick, TickStats { spent: 2, deferred: 3 });
    engine.tick();
    assert_eq!(ran(&engine), [ids[4], child]);
    engine.tick();
    assert_eq!(ran(&engine), [late]);
    engine.tick();
    assert_eq!(engine.last_tick, TickStats::default());
}

#[test]
fn oversized_activation_runs_alone_without_starvation() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(2);
    let large = probe(&mut engine, 9, false, false);
    engine.trigger_node(large, PinId(0));
    let next = queued(&mut engine, 1)[0];
    engine.tick();
    assert_eq!(ran(&engine), [large]);
    assert_eq!(engine.last_tick, TickStats { spent: 9, deferred: 1 });
    engine.tick();
    assert_eq!(ran(&engine), [next]);
}

#[test]
fn task_deliveries_use_budget_before_queue_even_when_over_limit() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(3);
    let owner = probe(&mut engine, 2, false, false);
    engine.host_state.start_delay(owner, 1);
    let ids = queued(&mut engine, 3);
    engine.tick();
    assert_eq!(ran(&engine), [owner, ids[0]]);
    assert_eq!(engine.last_tick, TickStats { spent: 3, deferred: 2 });
    engine.host_state.start_delay(owner, 1);
    engine.host_state.start_delay(owner, 1);
    engine.tick();
    assert_eq!(ran(&engine), [owner]);
    assert_eq!(engine.last_tick, TickStats { spent: 4, deferred: 2 });
}

#[test]
fn polling_costs_work_even_when_source_does_not_fire() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(3);
    probe(&mut engine, 2, false, true);
    let ids = queued(&mut engine, 3);
    engine.tick();
    assert_eq!(ran(&engine), ids[..1]);
    assert_eq!(engine.last_tick, TickStats { spent: 3, deferred: 2 });
}

#[test]
fn pure_work_is_counted_without_interrupting_activation() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(2);
    let source = probe(&mut engine, 7, true, false);
    let sink = NodeId::generate();
    engine.create_node(sink, "test/debug", &[]).unwrap();
    engine.connect_data(source, PinId(0), sink, PinId(1)).unwrap();
    engine.trigger_node(sink, PinId(0));
    queued(&mut engine, 1);
    engine.tick();
    assert_eq!(ran(&engine), [source, sink]);
    assert_eq!(engine.last_tick, TickStats { spent: 8, deferred: 1 });
}

#[test]
fn unlimited_drains_burst_and_still_reports_work() {
    let mut engine = TickEngine::new();
    assert_eq!(engine.budget.limit, None);
    let ids = queued(&mut engine, 20);
    engine.tick();
    assert_eq!(ran(&engine), ids);
    assert!(engine.execution_queue.is_empty());
    assert_eq!(engine.last_tick, TickStats { spent: 20, deferred: 0 });
}

#[test]
fn accounting_saturates_and_budget_comparison_does_not_overflow() {
    let budget = TickBudget { limit: Some(u32::MAX) };
    assert!(budget.should_defer(u32::MAX, 1));
    assert!(!budget.should_defer(0, u32::MAX));
    let mut engine = TickEngine::new();
    for _ in 0..2 {
        let id = probe(&mut engine, u32::MAX, false, false);
        engine.trigger_node(id, PinId(0));
    }
    engine.tick();
    assert_eq!(engine.last_tick.spent, u32::MAX);
    assert_eq!(engine.last_tick.deferred, 0);
}

#[test]
fn removed_activation_is_discarded_without_spending_budget() {
    let mut engine = TickEngine::new();
    engine.budget.limit = Some(1);
    engine.trigger_node(NodeId::generate(), PinId(0));
    let id = queued(&mut engine, 1)[0];
    engine.tick();
    assert_eq!(ran(&engine), [id]);
    assert_eq!(engine.last_tick, TickStats { spent: 1, deferred: 0 });
}
