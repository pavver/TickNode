use std::sync::Arc;
use crate::*;
use ticknode_shared::*;

fn assert_definitions(engine: &mut TickEngine) {
    for id in engine.node_ids() {
        let cached = engine.definition_arc(id).unwrap();
        assert_eq!(*cached, engine.fresh_node_definition(id).unwrap(), "node {id:?}");
        assert!(Arc::ptr_eq(&cached, &engine.definition_arc(id).unwrap()), "cache miss for {id:?}");
    }
}

#[test]
fn definitions_follow_graph_edits_and_execution() {
    fn assert_send<T: Send>() {}
    assert_send::<TickEngine>();
    let mut engine = TickEngine::new();
    // Every operation checks all nodes and warms their caches before the next edit.
    macro_rules! step {
        ($operation:expr) => {{
            let result = $operation;
            assert_definitions(&mut engine);
            result
        }};
    }
    let source = NodeId::generate();
    let expression = NodeId::generate();
    let list = NodeId::generate();
    let add = NodeId::generate();
    let object = NodeId::generate();
    let sink = NodeId::generate();
    let clock = NodeId::generate();
    for (id, name) in [
        (source, "const"), (expression, "math/expression"), (list, "list/create"),
        (add, "list/add"), (object, "object/set_field"), (sink, "test/debug"), (clock, "clock"),
    ] {
        step!(engine.create_node(id, name, &[]).unwrap());
    }
    step!(engine.set_node_property(source, "value", "4".into()).unwrap());
    step!(engine.set_node_property(expression, "expression", "a + b".into()).unwrap());
    step!(engine.connect_data(source, PinId(0), expression, PinId(2)).unwrap());
    assert_eq!(step!(engine.set_node_property(expression, "expression", "a * 2".into()).unwrap()).data.len(), 1);
    step!(engine.connect_data(source, PinId(0), expression, PinId(1)).unwrap());
    step!(engine.connect_data(expression, PinId(0), sink, PinId(1)).unwrap());
    step!(engine.connect_exec(clock, PinId(0), sink, PinId(0)).unwrap());
    step!(engine.trigger_node(sink, PinId(0)));
    step!(engine.tick());
    assert!(engine.executed.iter().any(|trace| trace.node_id == expression));
    step!(engine.connect_data(list, PinId(0), add, PinId(0)).unwrap());
    step!(engine.set_node_property(add, "element_type", "string".into()).unwrap());
    step!(engine.set_node_property(list, "element_type", "string".into()).unwrap());
    step!(engine.connect_data(list, PinId(0), add, PinId(0)).unwrap());
    step!(engine.set_input_value(add, PinId(1), Some("item".into())).unwrap());
    step!(engine.pull_data(add, PinId(2), &mut vec![]));
    // A built-in with a named field type supports split pins after type registration.
    step!(engine.register_types(&[TypeDefinition::new_struct("cache.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
        FieldDefinition::new("label", "Label", DataType::String).optional(),
    ])], &[]).unwrap());
    step!(engine.set_node_property(object, "field_type", "cache.point".into()).unwrap());
    step!(engine.set_pin_split(object, PinId(2), true).unwrap());
    let field = split_field_pin(PinId(2), 0).unwrap();
    step!(engine.set_input_value(object, field, Some(3i64.into())).unwrap());
    step!(engine.connect_data(source, PinId(0), object, field).unwrap());
    step!(engine.pull_inputs(object, &mut vec![]));
    step!(engine.set_pin_split(object, PinId(2), false).unwrap());
    step!(engine.set_pin_split(object, PinId(2), true).unwrap());
    step!(engine.set_node_property(object, "field_type", "number".into()).unwrap());
    step!(engine.disconnect_data(sink, PinId(1)));
    step!(engine.disconnect_exec(clock, PinId(0), sink, PinId(0)));
    let snapshot = engine.export_graph();
    step!(engine.create_node(expression, "math/expression", &[("expression".into(), "x + y + z".into())]).unwrap());
    step!(engine.reconcile(&snapshot));
    step!(engine.load_graph(&snapshot));
    step!(engine.import_snippet(&snapshot));
    step!(engine.remove_node(source));
    assert!(engine.definition_arc(source).is_none());
    step!(engine.add_node(Box::new(CompareNode { id: expression })));
    step!(engine.tick());
}

#[test]
fn definitions_follow_plugin_trap_refresh_and_stub_revival() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(&path) else {
        assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} not found", path.display());
        return;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let cost = NodeId::generate();
    let crash = NodeId::generate();
    engine.create_node(cost, "test/cost", &[("cost".into(), 7i64.into())]).unwrap();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    assert_definitions(&mut engine);
    let before = engine.definition_arc(cost).unwrap();
    let revision = engine.nodes[&cost].definition_revision();
    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(engine.host_state.logs.iter().any(|(id, log)| *id == crash && log.contains("Plugin crashed")));
    assert_definitions(&mut engine);
    engine.tick();
    assert_definitions(&mut engine);
    // Bypass engine invalidation to exercise WasmNode's revision on refresh after recreation.
    engine.nodes.get_mut(&cost).unwrap().set_property("cost", 2i64.into());
    assert!(engine.nodes[&cost].definition_revision() > revision);
    assert_definitions(&mut engine);
    let after = engine.definition_arc(cost).unwrap();
    assert!(!Arc::ptr_eq(&before, &after));
    assert_eq!(after.cost, 2);
    engine.trigger_node(cost, PinId(0));
    engine.tick();
    assert_eq!(engine.last_tick.spent, 2);
    assert_definitions(&mut engine);
    engine.disable_plugin("test_plugin").unwrap();
    assert_definitions(&mut engine);
    assert!(engine.node_unavailable(cost).is_some());
    engine.enable_plugin("test_plugin", &[]).unwrap();
    assert_definitions(&mut engine);
    assert!(engine.node_unavailable(cost).is_none());
    assert_eq!(engine.definition_arc(cost).unwrap().cost, 2);
}
