use super::*;

#[test]
fn one_unit_of_prior_work_defers_an_oversized_activation() {
    let mut e = TickEngine::new();
    e.budget.limit = Some(1);
    let button = add(&mut e, "test/click_button");
    let expensive = add(&mut e, "list/add");
    e.trigger_node(button, PinId(0));
    e.trigger_node(expensive, PinId(0));
    e.tick();
    assert_eq!(
        e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(),
        [button]
    );
    assert_eq!(
        e.last_tick,
        TickStats {
            spent: 1,
            deferred: 1
        }
    );
    e.tick();
    assert_eq!(
        e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(),
        [expensive]
    );
    assert_eq!(
        e.last_tick,
        TickStats {
            spent: 5,
            deferred: 0
        }
    );
}

#[test]
fn generated_make_and_break_charge_for_pulled_work() {
    let mut e = TickEngine::new();
    point_type(&mut e);
    let make = add(&mut e, "type/make/test.point");
    let split = add(&mut e, "type/break/test.point");
    let console = add(&mut e, "console");
    e.set_input_value(make, PinId(1), Some(17i64.into()))
        .unwrap();
    e.connect_data(make, PinId(0), split, PinId(0)).unwrap();
    e.connect_data(split, PinId(1), console, PinId(1)).unwrap();
    e.trigger_node(console, PinId(0));
    e.tick();
    assert_eq!(e.host_state.logs, [(console, "17".into())]);
    assert_eq!(
        e.last_tick,
        TickStats {
            spent: 3,
            deferred: 0
        }
    );
    assert_eq!(
        e.executed
            .iter()
            .find(|t| t.node_id == make)
            .unwrap()
            .outputs,
        [(PinId(0), point(17))]
    );
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
    assert!(matches!(
        e.connect_data(c, PinId(2), a, PinId(1)),
        Err(EngineError::CycleDetected(..))
    ));
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
    assert_eq!(
        e.host_state.logs,
        [(console, "0".into()), (console, "0".into())]
    );
    assert_eq!(
        e.last_tick.spent, 4,
        "an exec pulse and an empty output list do not invalidate pure results"
    );
}

#[test]
fn variable_only_outputs_refresh_pure_reads_in_the_same_tick() {
    let mut e = TickEngine::new();
    let read = add(&mut e, "variable/get");
    e.set_node_property(read, "name", "my_var".into()).unwrap();
    e.set_node_property(read, "value_type", "number".into())
        .unwrap();
    e.variables
        .create(VariableKey::global("my_var"), DataType::Number, 0i64.into());
    let console = add(&mut e, "console");
    // Deliberately emit no data: a variable write alone must invalidate a prior read.
    let write = fixed(
        &mut e,
        vec![],
        vec![NodeOutput::Variable(VariableCommand::Set {
            name: "my_var".into(),
            lookup: VariableLookup::Auto,
            value: 23i64.into(),
        })],
    );
    e.connect_data(read, PinId(0), console, PinId(1)).unwrap();
    for id in [console, write, console] {
        e.trigger_node(id, PinId(0));
    }
    e.tick();
    assert_eq!(
        e.variables
            .get(&VariableKey::global("my_var"))
            .unwrap()
            .value,
        23i64.into()
    );
    assert_eq!(
        e.host_state.logs,
        [(console, "0".into()), (console, "23".into())]
    );
}

#[test]
fn data_only_outputs_refresh_pure_dependents_in_the_same_tick() {
    let mut e = TickEngine::new();
    let source = fixed(
        &mut e,
        vec![pin(7, PinDirection::Output, DataType::Number)],
        vec![NodeOutput::Data(PinId(7), 11i64.into())],
    );
    let sum = add(&mut e, "math/add");
    let console = add(&mut e, "console");
    e.connect_data(source, PinId(7), sum, PinId(0)).unwrap();
    e.connect_data(sum, PinId(2), console, PinId(1)).unwrap();
    for id in [console, source, console] {
        e.trigger_node(id, PinId(0));
    }
    e.tick();
    assert_eq!(e.data_cache[&source][&PinId(7)], 11i64.into());
    assert_eq!(
        e.host_state.logs,
        [(console, "0".into()), (console, "11".into())]
    );
}
