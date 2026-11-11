use super::*;

#[test]
fn test_built_in_logic_nodes() {
    let mut engine = TickEngine::new();

    // Match with a single case "a = true" on pin 2 — equivalent to the former Branch node
    let branch_id = NodeId::generate();
    engine.create_node(branch_id, "match", &[cases(&["2: a = true"])]).unwrap();

    // Compare
    let compare_id = NodeId::generate();
    let compare = engine.registry.create_node("compare", compare_id).unwrap();
    engine.add_node(compare);

    // Connect compare inputs to Const nodes
    let const_a_id = NodeId::generate();
    let const_a = Box::new(ConstNode { id: const_a_id, value: DataValue::Number(NumberValue::Int(10)), value_type: None });
    engine.add_node(const_a);

    let const_b_id = NodeId::generate();
    let const_b = Box::new(ConstNode { id: const_b_id, value: DataValue::Number(NumberValue::Int(5)), value_type: None });
    engine.add_node(const_b);

    engine.connect_data(const_a_id, PinId(0), compare_id, PinId(0)).unwrap();
    engine.connect_data(const_b_id, PinId(0), compare_id, PinId(1)).unwrap();

    // Pull compare outputs to verify
    let mut stack = Vec::new();
    let eq_val = engine.pull_data(compare_id, PinId(2), &mut stack).unwrap();
    let gt_val = engine.pull_data(compare_id, PinId(3), &mut stack).unwrap();
    let lt_val = engine.pull_data(compare_id, PinId(4), &mut stack).unwrap();

    assert_eq!(eq_val, DataValue::Bool(false));
    assert_eq!(gt_val, DataValue::Bool(true));
    assert_eq!(lt_val, DataValue::Bool(false));

    // Compare Gt → input a of the Match node
    engine.connect_data(compare_id, PinId(3), branch_id, PinId(1)).unwrap();

    // Trigger Match
    engine.trigger_node(branch_id, PinId(0));

    // Case 2 → sink
    let target_id = add_sink(&mut engine);
    engine.connect_exec(branch_id, PinId(2), target_id, PinId(0)).unwrap();

    engine.tick();
    // Gt = true, so pin 2 fires and the sink is queued
    assert!(engine.execution_queue.contains(&(target_id, PinId(0))));
}

#[test]
fn test_built_in_for_loop_and_sleep() {
    let mut engine = TickEngine::new();

    // ForLoop
    let loop_id = NodeId::generate();
    let for_loop = engine.registry.create_node("for_loop", loop_id).unwrap();
    engine.add_node(for_loop);

    // Connect Loop Count to Const
    let const_count_id = NodeId::generate();
    let const_count = Box::new(ConstNode { id: const_count_id, value: DataValue::Number(NumberValue::Int(3)), value_type: None });
    engine.add_node(const_count);
    engine.connect_data(const_count_id, PinId(0), loop_id, PinId(1)).unwrap();

    // Connect LoopBody and Completed to dummy target nodes to trace exec flow
    let dummy_body_id = add_sink(&mut engine);
    let dummy_completed_id = add_sink(&mut engine);
    engine.connect_exec(loop_id, PinId(4), dummy_body_id, PinId(0)).unwrap();
    engine.connect_exec(loop_id, PinId(6), dummy_completed_id, PinId(0)).unwrap();

    // Trigger In (Pin 0)
    engine.trigger_node(loop_id, PinId(0));
    engine.tick();

    // It should output LoopBody (Pin 4) -> should trigger dummy_body
    assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(0))));
    assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
    engine.execution_queue.clear();

    // Trigger Next (Pin 3)
    engine.trigger_node(loop_id, PinId(3));
    engine.tick();
    // Index = 1, LoopBody
    assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(1))));
    assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
    engine.execution_queue.clear();

    // Trigger Next (Pin 3)
    engine.trigger_node(loop_id, PinId(3));
    engine.tick();
    // Index = 2, LoopBody
    assert_eq!(engine.data_cache.get(&loop_id).unwrap().get(&PinId(5)), Some(&DataValue::Number(NumberValue::Int(2))));
    assert!(engine.execution_queue.contains(&(dummy_body_id, PinId(0))));
    engine.execution_queue.clear();

    // Trigger Next (Pin 3)
    engine.trigger_node(loop_id, PinId(3));
    engine.tick();
    // Completed (Pin 6) -> should trigger dummy_completed
    assert!(engine.execution_queue.contains(&(dummy_completed_id, PinId(0))));
    engine.execution_queue.clear();

    // --- Sleep Node ---
    let sleep_id = NodeId::generate();
    let sleep_node = engine.registry.create_node("sleep", sleep_id).unwrap();
    engine.add_node(sleep_node);

    // Connect Sleep inputs
    let const_delay_id = NodeId::generate();
    let const_delay = Box::new(ConstNode { id: const_delay_id, value: DataValue::Ticks(3), value_type: None });
    engine.add_node(const_delay);
    engine.connect_data(const_delay_id, PinId(0), sleep_id, PinId(1)).unwrap();

    // Connect Sleep Out to dummy target node to trace exec flow
    let dummy_sleep_out_id = add_sink(&mut engine);
    engine.connect_exec(sleep_id, PinId(4), dummy_sleep_out_id, PinId(0)).unwrap();

    engine.trigger_node(sleep_id, PinId(0)); // trigger In
    engine.tick(); // task created

    assert_eq!(engine.host_state.active_tasks.len(), 1);
    let task_handle = *engine.host_state.active_tasks.keys().next().unwrap();

    engine.tick(); // elapsed = 1
    assert!(engine.host_state.active_tasks.contains_key(&task_handle));

    engine.tick(); // elapsed = 2
    assert!(engine.host_state.active_tasks.contains_key(&task_handle));

    engine.tick(); // elapsed = 3 -> task finished and delivered to the node, Out queues the next node
    assert!(!engine.host_state.active_tasks.contains_key(&task_handle));
    assert!(engine.execution_queue.contains(&(dummy_sleep_out_id, PinId(0))));
}

#[test]
fn test_built_in_collections() {
    let mut engine = TickEngine::new();

    // 1. List
    let create_id = NodeId::generate();
    let mut create_node = engine.registry.create_node("list/create", create_id).unwrap();
    create_node.set_property("element_type", DataValue::String("number".to_string()));
    engine.add_node(create_node);

    let add_id = NodeId::generate();
    let mut add_node = engine.registry.create_node("list/add", add_id).unwrap();
    add_node.set_property("element_type", DataValue::String("number".to_string()));
    engine.add_node(add_node);

    // Connect List from create to add
    engine.connect_data(create_id, PinId(0), add_id, PinId(0)).unwrap();

    // Connect Item to Const
    let const_item_id = NodeId::generate();
    let const_item = Box::new(ConstNode { id: const_item_id, value: DataValue::Number(NumberValue::Int(42)), value_type: None });
    engine.add_node(const_item);
    engine.connect_data(const_item_id, PinId(0), add_id, PinId(1)).unwrap();

    // Pull output of add_node
    let mut stack = Vec::new();
    let res_list = engine.pull_data(add_id, PinId(2), &mut stack).unwrap();
    if let DataValue::List { element_type, elements } = res_list {
        assert_eq!(element_type, DataType::Number);
        assert_eq!(elements, vec![DataValue::Number(NumberValue::Int(42))]);
    } else {
        panic!("Expected list output");
    }

    // 2. Object
    let obj_create_id = NodeId::generate();
    let obj_create = engine.registry.create_node("object/create", obj_create_id).unwrap();
    engine.add_node(obj_create);

    let obj_set_id = NodeId::generate();
    let mut obj_set = engine.registry.create_node("object/set_field", obj_set_id).unwrap();
    obj_set.set_property("field_type", DataValue::String("number".to_string()));
    engine.add_node(obj_set);

    // Connect obj_create to obj_set
    engine.connect_data(obj_create_id, PinId(0), obj_set_id, PinId(0)).unwrap();

    // Connect Name and Val to Const nodes
    let const_name_id = NodeId::generate();
    let const_name = Box::new(ConstNode { id: const_name_id, value: DataValue::String("age".to_string()), value_type: None });
    engine.add_node(const_name);
    engine.connect_data(const_name_id, PinId(0), obj_set_id, PinId(1)).unwrap();

    let const_val_id = NodeId::generate();
    let const_val = Box::new(ConstNode { id: const_val_id, value: DataValue::Number(NumberValue::Int(30)), value_type: None });
    engine.add_node(const_val);
    engine.connect_data(const_val_id, PinId(0), obj_set_id, PinId(2)).unwrap();

    // Pull object
    stack.clear();
    let res_obj = engine.pull_data(obj_set_id, PinId(3), &mut stack).unwrap();

    // Now Unpack
    let unpack_id = NodeId::generate();
    let mut unpack = engine.registry.create_node("object/unpack", unpack_id).unwrap();
    unpack.set_property("fields_list", DataValue::String("age:number,name:string".to_string()));
    engine.add_node(unpack);

    // Set unpack Obj input by creating a Const node wrapper for res_obj
    let const_obj_id = NodeId::generate();
    let const_obj = Box::new(ConstNode { id: const_obj_id, value: res_obj, value_type: None });
    engine.add_node(const_obj);
    engine.connect_data(const_obj_id, PinId(0), unpack_id, PinId(0)).unwrap();

    // Pull age (pin 1) and name (pin 2)
    stack.clear();
    let age_val = engine.pull_data(unpack_id, PinId(1), &mut stack).unwrap();
    let name_val = engine.pull_data(unpack_id, PinId(2), &mut stack); // name is missing, should be None

    assert_eq!(age_val, DataValue::Number(NumberValue::Int(30)));
    assert!(name_val.is_none());
}

#[test]
fn test_click_button_and_debug_nodes() {
    let mut engine = TickEngine::new();

    // 1. Create ClickButton
    let button_id = NodeId::generate();
    let button_node = engine.registry.create_node("test/click_button", button_id).unwrap();
    engine.add_node(button_node);

    // 2. Create DebugNode and set prefix
    let debug_id = NodeId::generate();
    let mut debug_node = engine.registry.create_node("test/debug", debug_id).unwrap();
    debug_node.set_property("prefix", DataValue::String("variable value: ".to_string()));
    engine.add_node(debug_node);

    // 3. Connect Exec: Button Out -> Debug In
    engine.connect_exec(button_id, PinId(0), debug_id, PinId(0)).unwrap();

    // 4. Create ConstNode (444) and connect to Debug Val (Any)
    let const_id = NodeId::generate();
    let const_node = Box::new(ConstNode { id: const_id, value: DataValue::Number(NumberValue::Int(444)), value_type: None });
    engine.add_node(const_node);
    engine.connect_data(const_id, PinId(0), debug_id, PinId(1)).unwrap(); // Any connection must succeed!

    // Simulate user click
    engine.trigger_node(button_id, PinId(0));

    // First tick: Button executes, debug node is queued for the next tick
    engine.tick();
    assert!(engine.execution_queue.contains(&(debug_id, PinId(0))));

    // Second tick: Debug node executes
    engine.tick();
    assert!(engine.execution_queue.is_empty());
}

#[test]
fn test_graph_roundtrip_keeps_properties_and_connections() {
    let mut engine = TickEngine::new();
    let a = NodeId::generate();
    let b = NodeId::generate();
    let dbg = NodeId::generate();
    engine.create_node(a, "const", &[("value".into(), DataValue::String("hi".into()))]).unwrap();
    engine.create_node(b, "math/expression", &[("expression".into(), DataValue::String("x * 2".into()))]).unwrap();
    engine.create_node(dbg, "test/debug", &[("prefix".into(), DataValue::String("> ".into()))]).unwrap();
    engine.connect_data(a, PinId(0), dbg, PinId(1)).unwrap();

    let graph = engine.export_graph();
    let mut restored = TickEngine::new();
    let warnings = restored.load_graph(&graph);
    assert!(warnings.is_empty(), "{:?}", warnings);
    assert_eq!(restored.export_graph(), graph);
    // Formula pin was restored from the expression
    let def = restored.node_definition(b).unwrap();
    assert!(def.pins.iter().any(|p| p.name == "x" && p.direction == PinDirection::Input));
}

#[test]
fn test_data_input_has_single_source_and_exec_is_validated() {
    let mut engine = TickEngine::new();
    let c1 = NodeId::generate();
    let c2 = NodeId::generate();
    let dbg = add_sink(&mut engine);
    engine.create_node(c1, "const", &[]).unwrap();
    engine.create_node(c2, "const", &[]).unwrap();
    engine.connect_data(c1, PinId(0), dbg, PinId(1)).unwrap();
    engine.connect_data(c2, PinId(0), dbg, PinId(1)).unwrap();
    assert_eq!(engine.data_connections().len(), 1);
    assert_eq!(engine.data_connections()[0].src_node, c2);

    // Data pin cannot be connected as exec
    assert_eq!(engine.connect_exec(c1, PinId(0), dbg, PinId(0)), Err(EngineError::InvalidPinConnection));
}

#[test]
fn test_property_change_prunes_invalid_connections() {
    let mut engine = TickEngine::new();
    let x = NodeId::generate();
    let expr = NodeId::generate();
    engine.create_node(x, "const", &[]).unwrap();
    engine.create_node(expr, "math/expression", &[("expression".into(), DataValue::String("a + b".into()))]).unwrap();
    engine.connect_data(x, PinId(0), expr, PinId(2)).unwrap(); // b

    // Formula now has only one variable — pin 2 no longer exists
    let removed = engine.set_node_property(expr, "expression", DataValue::String("a * 3".into())).unwrap();
    assert_eq!(removed.data.len(), 1);
    assert!(engine.data_connections().is_empty());
}

#[test]
fn test_remove_node_cancels_its_tasks() {
    let mut engine = TickEngine::new();
    let sleep = NodeId::generate();
    engine.create_node(sleep, "sleep", &[]).unwrap();
    let delay = NodeId::generate();
    engine.create_node(delay, "const", &[("value".into(), DataValue::Ticks(100))]).unwrap();
    engine.connect_data(delay, PinId(0), sleep, PinId(1)).unwrap();
    engine.trigger_node(sleep, PinId(0));
    engine.tick();
    assert_eq!(engine.host_state.active_tasks.len(), 1);

    engine.remove_node(sleep);
    assert!(engine.host_state.active_tasks.is_empty());
}

#[test]
fn test_clock_fires_every_interval() {
    let mut engine = TickEngine::new();
    let clock = NodeId::generate();
    engine.create_node(clock, "clock", &[("interval_ticks".into(), DataValue::Number(NumberValue::Int(3)))]).unwrap();
    let sink = add_sink(&mut engine);
    engine.connect_exec(clock, PinId(0), sink, PinId(0)).unwrap();

    let mut fired_on = Vec::new();
    for _ in 0..9 {
        engine.tick();
        if engine.executed.iter().any(|t| t.node_id == clock) {
            fired_on.push(engine.host_state.current_tick);
        }
    }
    assert_eq!(fired_on, vec![3, 6, 9]);
}

#[test]
fn test_traces_record_inputs_outputs_and_fired_pins() {
    let mut engine = TickEngine::new();
    let num = |v| DataValue::Number(NumberValue::Int(v));
    let (a, b, add, debug) = (NodeId::generate(), NodeId::generate(), NodeId::generate(), NodeId::generate());
    engine.create_node(a, "const", &[("value".into(), num(2))]).unwrap();
    engine.create_node(b, "const", &[("value".into(), num(3))]).unwrap();
    engine.create_node(add, "math/add", &[]).unwrap();
    engine.create_node(debug, "test/debug", &[]).unwrap();
    engine.connect_data(a, PinId(0), add, PinId(0)).unwrap();
    engine.connect_data(b, PinId(0), add, PinId(1)).unwrap();
    engine.connect_data(add, PinId(2), debug, PinId(1)).unwrap();

    engine.trigger_node(debug, PinId(0));
    engine.tick();

    let trace = |id| engine.executed.iter().find(|t| t.node_id == id).cloned().unwrap();
    let d = trace(debug);
    assert_eq!(d.tick, 1);
    assert_eq!(d.inputs, vec![(PinId(1), Some(num(5)))]);
    assert_eq!(d.fired, vec![PinId(2)]);
    // Pure node evaluated on demand also produces a trace.
    let s = trace(add);
    assert_eq!(s.inputs, vec![(PinId(0), Some(num(2))), (PinId(1), Some(num(3)))]);
    assert_eq!(s.outputs, vec![(PinId(2), num(5))]);

    engine.tick();
    assert!(engine.executed.is_empty());
}

#[test]
fn test_import_snippet_maps_ids_and_keeps_inner_connections() {
    let mut engine = TickEngine::new();
    let (a, b, outside) = (NodeId::generate(), NodeId::generate(), NodeId::generate());
    engine.create_node(a, "const", &[]).unwrap();
    engine.create_node(b, "test/debug", &[]).unwrap();
    engine.create_node(outside, "test/debug", &[]).unwrap();
    engine.connect_data(a, PinId(0), b, PinId(1)).unwrap();
    engine.connect_data(a, PinId(0), outside, PinId(1)).unwrap();
    let mut snippet = engine.export_snippet(&[a, b]);
    // Connection to a node outside the snippet (e.g., from another graph's clipboard) is ignored.
    snippet.data_connections.push(ConnectionModel { src_node: a, src_pin: PinId(0), dst_node: outside, dst_pin: PinId(1) });

    let (created, warnings) = engine.import_snippet(&snippet);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(created.iter().map(|(old, _)| *old).collect::<Vec<_>>(), vec![a, b]);
    let (new_a, new_b) = (created[0].1, created[1].1);
    assert!(new_a != a && new_b != b);
    assert_eq!(engine.export_snippet(&[new_a, new_b]).data_connections.len(), 1);
    assert_eq!(engine.node_ids().len(), 5);
}
