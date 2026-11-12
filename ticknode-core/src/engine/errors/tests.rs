use super::*;
use crate::{Node, NodeContext};
use ticknode_shared::*;

struct Probe {
    id: NodeId,
    required: bool,
    fail: bool,
    pulse: bool,
}

impl Node for Probe {
    fn id(&self) -> NodeId {
        self.id
    }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            type_name: "test/status".into(),
            title: "Status".into(),
            category: "Test".into(),
            description: String::new(),
            help: String::new(),
            properties: vec![],
            pins: vec![PinDefinition {
                id: PinId(1),
                name: "Value".into(),
                direction: PinDirection::Input,
                pin_type: PinType::Data(DataType::Number),
                required: self.required,
            }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if self.fail {
            ctx.host_state.error(
                self.id,
                UiMessage::plain("Probe failed"),
                "[ERROR] Probe failed".into(),
            );
        }
        vec![]
    }
    fn polls(&self) -> bool {
        self.pulse
    }
    fn poll(&mut self, _: &mut crate::HostState) -> Option<PinId> {
        self.pulse.then_some(PinId(0))
    }
    fn set_property(&mut self, _: &str, _: DataValue) {
        self.fail = false;
    }
}

fn probe(required: bool, fail: bool) -> (TickEngine, NodeId) {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.add_node(Box::new(Probe {
        id,
        required,
        fail,
        pulse: false,
    }));
    (engine, id)
}

fn fail(engine: &mut TickEngine, id: NodeId) {
    engine
        .host_state
        .error(id, UiMessage::plain("Failure"), "[ERROR] Failure".into());
}

#[test]
fn missing_required_input_sets_error_until_a_value_arrives_and_execution_succeeds() {
    let (mut engine, id) = probe(true, false);
    engine.trigger_node(id, PinId(0));
    engine.tick();
    let (set, cleared) = engine.take_node_error_changes();
    assert!(cleared.is_empty());
    assert_eq!(set.len(), 1);
    assert_eq!(set[0].message.key, "error-node-required-missing");
    assert_eq!(set[0].message.args, [("pin".into(), "Value".into())]);
    assert_eq!(set[0].count, 1);
    assert!(engine.take_node_error_changes().0.is_empty());

    let source = NodeId::generate();
    engine.create_node(source, "for_loop", &[]).unwrap();
    engine.connect_data(source, PinId(5), id, PinId(1)).unwrap();
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(
        engine.node_errors().len(),
        1,
        "a connected source without a value does not clear an error"
    );
    engine
        .data_cache
        .entry(source)
        .or_default()
        .insert(PinId(5), 42i64.into());
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert!(engine.node_errors().is_empty());
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![id]));
}

#[test]
fn failed_execution_keeps_error_and_successful_execution_clears_it() {
    let (mut engine, id) = probe(false, true);
    for count in 1..=3 {
        engine.trigger_node(id, PinId(0));
        engine.tick();
        assert_eq!(engine.node_errors()[0].count, count);
    }
    let (set, cleared) = engine.take_node_error_changes();
    assert_eq!(set.len(), 1);
    assert_eq!(set[0].count, 3);
    assert!(cleared.is_empty());
    assert!(engine.take_node_error_changes().0.is_empty());
    assert_eq!(
        engine.host_state.logs.len(),
        3,
        "structured reporting preserves logs"
    );
    // Change runtime behavior without a user edit to test success-driven clearing itself.
    engine
        .nodes
        .get_mut(&id)
        .unwrap()
        .set_property("fail", false.into());
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![id]));
}

#[test]
fn property_and_input_edits_and_removal_clear_errors() {
    let (mut engine, id) = probe(false, false);
    fail(&mut engine, id);
    engine.tick();
    engine.take_node_error_changes();
    engine.set_node_property(id, "fail", false.into()).unwrap();
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![id]));
    fail(&mut engine, id);
    engine.tick();
    engine
        .set_input_value(id, PinId(1), Some(3i64.into()))
        .unwrap();
    assert!(engine.node_errors().is_empty());
    fail(&mut engine, id);
    engine.tick();
    engine.remove_node(id);
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![id]));
    fail(&mut engine, id);
    engine.tick();
    assert!(
        engine.node_errors().is_empty(),
        "removed owners cannot acquire errors again"
    );
}

#[test]
fn nil_id_is_logged_but_is_never_a_node_error() {
    let mut engine = TickEngine::new();
    fail(&mut engine, NodeId(Uuid::nil()));
    engine.tick();
    assert_eq!(engine.host_state.logs.len(), 1);
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![]));
}

#[test]
fn any_error_in_the_tick_wins_over_success_in_the_same_tick() {
    let (mut engine, id) = probe(false, false);
    fail(&mut engine, id);
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(engine.node_errors().len(), 1);
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert!(engine.node_errors().is_empty());
}

#[test]
fn required_conversion_errors_clear_when_the_source_value_recovers() {
    let (mut engine, id) = probe(true, false);
    let source = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), "invalid".into())])
        .unwrap();
    engine.connect_data(source, PinId(0), id, PinId(1)).unwrap();
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert_eq!(
        engine.node_errors()[0].message.key,
        "error-node-required-conversion"
    );
    engine
        .set_node_property(source, "value", "42".into())
        .unwrap();
    engine.trigger_node(id, PinId(0));
    engine.tick();
    assert!(engine.node_errors().is_empty());
}

#[test]
fn task_delivery_and_pure_evaluation_clear_errors() {
    let (mut engine, id) = probe(false, false);
    fail(&mut engine, id);
    engine.tick();
    engine.host_state.start_delay(id, 1);
    engine.tick();
    assert!(engine.node_errors().is_empty());
    engine.take_node_error_changes();
    let source = NodeId::generate();
    engine.create_node(source, "const", &[]).unwrap();
    fail(&mut engine, source);
    engine.tick();
    assert_eq!(engine.node_errors().len(), 1);
    engine.pull_data(source, PinId(0), &mut vec![]);
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![source]));
}

#[test]
fn settings_paste_clears_even_when_the_graph_is_unchanged() {
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.create_node(id, "const", &[]).unwrap();
    fail(&mut engine, id);
    engine.tick();
    engine.take_node_error_changes();
    let graph = engine.export_graph();
    engine.reconcile_settings(&graph, id);
    assert_eq!(engine.take_node_error_changes(), (vec![], vec![id]));
}

#[test]
fn idle_poll_preserves_error_and_a_pulse_clears_it() {
    struct Source {
        id: NodeId,
        pulse: bool,
    }
    impl Node for Source {
        fn id(&self) -> NodeId {
            self.id
        }
        fn definition(&self) -> NodeDefinition {
            Probe {
                id: self.id,
                required: false,
                fail: false,
                pulse: true,
            }
            .definition()
        }
        fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
            vec![]
        }
        fn polls(&self) -> bool {
            true
        }
        fn poll(&mut self, _: &mut crate::HostState) -> Option<PinId> {
            self.pulse.then_some(PinId(0))
        }
        fn set_property(&mut self, _: &str, _: DataValue) {
            self.pulse = true;
        }
    }
    let mut engine = TickEngine::new();
    let id = NodeId::generate();
    engine.add_node(Box::new(Source { id, pulse: false }));
    fail(&mut engine, id);
    engine.tick();
    engine.tick();
    assert_eq!(engine.node_errors().len(), 1);
    // Bypass the user edit path to isolate recovery caused by the pulse.
    engine
        .nodes
        .get_mut(&id)
        .unwrap()
        .set_property("pulse", true.into());
    engine.tick();
    assert!(engine.node_errors().is_empty());
}
