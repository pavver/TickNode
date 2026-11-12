use super::*;

#[test]
fn split_traces_project_outputs_and_hide_the_parent_even_with_shared_ids() {
    let mut e = TickEngine::new();
    point_type(&mut e);
    let source = fixed(
        &mut e,
        vec![
            pin(0, PinDirection::Input, DataType::Number),
            pin(
                0,
                PinDirection::Output,
                DataType::Named("test.point".into()),
            ),
            pin(3, PinDirection::Output, DataType::Number),
        ],
        vec![
            NodeOutput::Data(PinId(0), point(29)),
            NodeOutput::Data(PinId(3), 99i64.into()),
        ],
    );
    e.set_input_value(source, PinId(0), Some(5i64.into()))
        .unwrap();
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
    assert_eq!(
        trace.outputs,
        [(PinId(3), 99i64.into()), (field, 29i64.into())]
    );
}

#[test]
fn conversions_use_the_output_type_when_input_and_output_ids_overlap() {
    let mut e = TickEngine::new();
    let source = fixed(
        &mut e,
        vec![
            pin(0, PinDirection::Input, DataType::Number),
            pin(0, PinDirection::Output, DataType::String),
        ],
        vec![NodeOutput::Data(PinId(0), "31".into())],
    );
    e.variables
        .create(VariableKey::global("my_var"), DataType::Number, 0i64.into());
    let write = add(&mut e, "variable/set");
    e.set_node_property(write, "name", "my_var".into()).unwrap();
    e.set_node_property(write, "value_type", "number".into())
        .unwrap();
    e.connect_data(source, PinId(0), write, PinId(3)).unwrap();
    e.trigger_node(source, PinId(0));
    e.trigger_node(write, PinId(0));
    e.tick();
    assert_eq!(
        e.variables
            .get(&VariableKey::global("my_var"))
            .unwrap()
            .value,
        31i64.into()
    );
    assert_eq!(
        e.executed
            .iter()
            .find(|t| t.node_id == write)
            .unwrap()
            .inputs,
        [(PinId(3), Some(31i64.into()))]
    );
}

#[test]
fn remove_node_removes_only_incident_exec_and_data_connections() {
    let mut e = TickEngine::new();
    e.variables
        .create(VariableKey::global("my_var"), DataType::Number, 0i64.into());
    let ids: Vec<_> = (0..4)
        .map(|_| {
            let id = add(&mut e, "variable/set");
            e.set_node_property(id, "name", "my_var".into()).unwrap();
            e.set_node_property(id, "value_type", "number".into())
                .unwrap();
            id
        })
        .collect();
    let values: Vec<_> = (0..3).map(|_| add(&mut e, "const")).collect();
    for (src, dst) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[3])] {
        e.connect_exec(src, PinId(1), dst, PinId(0)).unwrap();
    }
    for (src, dst) in [
        (values[0], ids[0]),
        (values[1], ids[1]),
        (values[2], ids[2]),
    ] {
        e.connect_data(src, PinId(0), dst, PinId(3)).unwrap();
    }
    e.set_input_value(ids[3], PinId(3), Some(0i64.into()))
        .unwrap();
    // Remove a middle exec node/data destination, then a data source.
    let removed = e.remove_node(ids[1]);
    assert_eq!(removed.exec.len(), 2);
    assert_eq!(removed.data.len(), 1);
    assert_eq!(
        e.exec_connections(),
        [ConnectionModel {
            src_node: ids[2],
            src_pin: PinId(1),
            dst_node: ids[3],
            dst_pin: PinId(0)
        }]
    );
    assert_eq!(e.data_connections().len(), 2);
    let removed = e.remove_node(values[0]);
    assert_eq!(
        removed.data,
        [ConnectionModel {
            src_node: values[0],
            src_pin: PinId(0),
            dst_node: ids[0],
            dst_pin: PinId(3)
        }]
    );
    assert_eq!(
        e.data_connections(),
        [ConnectionModel {
            src_node: values[2],
            src_pin: PinId(0),
            dst_node: ids[2],
            dst_pin: PinId(3)
        }]
    );
    e.trigger_node(ids[2], PinId(0));
    e.tick();
    e.tick();
    assert_eq!(
        e.executed.iter().map(|t| t.node_id).collect::<Vec<_>>(),
        [ids[3]]
    );
    assert_eq!(
        e.variables
            .get(&VariableKey::global("my_var"))
            .unwrap()
            .value,
        0i64.into()
    );
}

#[test]
fn changing_a_source_type_prunes_incompatible_outgoing_connections() {
    let mut e = TickEngine::new();
    let source = add(&mut e, "const");
    let sum = add(&mut e, "math/add");
    e.set_input_value(sum, PinId(0), Some(7i64.into())).unwrap();
    e.connect_data(source, PinId(0), sum, PinId(0)).unwrap();
    let removed = e
        .set_node_property(
            source,
            "value",
            DataValue::Object {
                fields: Default::default(),
                type_name: None,
            },
        )
        .unwrap();
    assert_eq!(
        removed.data,
        [ConnectionModel {
            src_node: source,
            src_pin: PinId(0),
            dst_node: sum,
            dst_pin: PinId(0)
        }]
    );
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
    assert_eq!(
        e.executed
            .iter()
            .find(|t| t.node_id == source)
            .unwrap()
            .outputs,
        [(PinId(0), 0i64.into())]
    );
}

#[test]
fn a_new_loop_ignores_next_and_break_until_started() {
    let mut e = TickEngine::new();
    let loop_id = add(&mut e, "for_loop");
    let console = add(&mut e, "console");
    e.connect_exec(loop_id, PinId(6), console, PinId(0))
        .unwrap();
    for pin in [3, 2] {
        e.trigger_node(loop_id, PinId(pin));
    }
    e.tick();
    assert_eq!(e.executed.len(), 1);
    assert_eq!(e.executed[0].node_id, loop_id);
    assert!(e.executed[0].fired.is_empty());
    e.tick();
    assert!(e.host_state.logs.is_empty());
    e.set_input_value(loop_id, PinId(1), Some(2i64.into()))
        .unwrap();
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
    e.set_input_value(source, PinId(1), Some(41i64.into()))
        .unwrap();
    e.variables
        .create(VariableKey::global("seen"), DataType::Bool, false.into());
    let sink = fixed(
        &mut e,
        vec![
            pin(0, PinDirection::Input, DataType::Named("test.point".into())),
            pin(1, PinDirection::Input, DataType::Named("test.point".into())),
        ],
        vec![NodeOutput::Variable(VariableCommand::Set {
            name: "seen".into(),
            lookup: VariableLookup::Auto,
            value: true.into(),
        })],
    );
    e.connect_data(source, PinId(0), sink, PinId(0)).unwrap();
    e.set_pin_split(sink, PinId(1), true).unwrap();
    let field = split_field_pin(PinId(1), 0).unwrap();
    e.set_input_value(sink, field, Some(43i64.into())).unwrap();
    e.trigger_node(sink, PinId(0));
    e.tick();
    let trace = e.executed.iter().find(|t| t.node_id == sink).unwrap();
    assert_eq!(
        trace.inputs,
        [
            (PinId(0), Some(point(41))),
            (PinId(1), Some(point(43))),
            (field, Some(43i64.into()))
        ]
    );
    assert_eq!(
        e.variables.get(&VariableKey::global("seen")).unwrap().value,
        true.into()
    );
}
