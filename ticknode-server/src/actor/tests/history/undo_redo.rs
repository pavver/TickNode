use super::*;

#[test]
fn history_add_undo_redo_same_id_and_protocol_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    state(&actor, false, false);
    let id = add(&mut actor, 12.0);
    state(&actor, true, false);
    let events = apply(&mut actor, ClientMessage::Undo);
    assert!(matches!(events.first(), Some(ServerMessage::NodeRemoved(n)) if *n == id));
    assert!(matches!(
        events.last(),
        Some(ServerMessage::History(HistoryState {
            can_undo: false,
            can_redo: true
        }))
    ));
    state(&actor, false, true);
    assert!(!actor.engine.has_node(id));
    let events = apply(&mut actor, ClientMessage::Redo);
    assert!(
        matches!(events.first(), Some(ServerMessage::NodeAdded(n)) if n.model.id == id && n.model.position.x == 12.0)
    );
    state(&actor, true, false);
    for event in events
        .into_iter()
        .chain([actor.snapshot(actor.sheets.first().map(|s| s.id))])
    {
        assert_eq!(
            postcard::from_bytes::<ServerMessage>(&encode(&event)).unwrap(),
            event
        );
    }
}

#[test]
fn history_new_edit_clears_redo_but_failures_and_noops_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let id = add(&mut actor, 10.0);
    apply(&mut actor, ClientMessage::Undo);
    let (reply, _) = mpsc::unbounded_channel();
    assert!(
        actor
            .apply(
                ClientMessage::RemoveNode(id),
                actor.sheets.first().map(|s| s.id),
                &reply
            )
            .is_err()
    );
    apply(
        &mut actor,
        ClientMessage::DisconnectData {
            dst_node: id,
            dst_pin: PinId(1),
        },
    );
    state(&actor, false, true);
    let id = add(&mut actor, 40.0);
    state(&actor, true, false);
    apply(
        &mut actor,
        ClientMessage::MoveNode {
            id,
            position: actor_position(40.0),
        },
    );
    apply(&mut actor, ClientMessage::Undo);
    assert!(
        !actor.engine.has_node(id),
        "an unchanged move must not add history"
    );
}

#[test]
fn history_plugin_toggle_clears_both_stacks() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
    std::fs::copy(wasm, dir.path().join("plugins/test.wasm")).unwrap();
    actor.plugins.load_all(&mut actor.engine);
    let name = actor.plugins.infos(&actor.engine)[0].name.clone();
    let events = apply(
        &mut actor,
        ClientMessage::ConfigurePlugin {
            name: name.clone(),
            values: vec![(
                "greeting".into(),
                SettingUpdate::Set(DataValue::from("Hello actor")),
            )],
        },
    );
    assert!(
        matches!(&events[0], ServerMessage::PluginUpdated(info) if info.values.contains(&(
            "greeting".into(), SettingValueView::Value(DataValue::from("Hello actor")),
        )))
    );
    add(&mut actor, 1.0);
    add(&mut actor, 2.0);
    apply(&mut actor, ClientMessage::Undo);
    state(&actor, true, true);
    let events = apply(
        &mut actor,
        ClientMessage::SetPluginEnabled {
            name,
            enabled: false,
        },
    );
    state(&actor, false, false);
    assert!(matches!(
        events.last(),
        Some(ServerMessage::History(HistoryState {
            can_undo: false,
            can_redo: false
        }))
    ));
}

#[test]
fn history_bounds_entries_and_empty_undo_redo_do_not_emit_events() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    assert!(apply(&mut actor, ClientMessage::Undo).is_empty());
    assert!(apply(&mut actor, ClientMessage::Redo).is_empty());
    let first = add(&mut actor, 0.0);
    for x in 1..=100 {
        add(&mut actor, x as f32);
    }
    for _ in 0..100 {
        apply(&mut actor, ClientMessage::Undo);
    }
    state(&actor, false, true);
    assert_eq!(actor.engine.node_ids(), vec![first]);
    assert!(apply(&mut actor, ClientMessage::Undo).is_empty());
    for _ in 0..100 {
        apply(&mut actor, ClientMessage::Redo);
    }
    state(&actor, true, false);
    assert_eq!(actor.engine.node_ids().len(), 101);
}

#[test]
fn history_moves_kept_nodes_and_recreates_changed_nodes_in_event_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = actor(dir.path());
    let a = add(&mut actor, 1.0);
    let b = add(&mut actor, 2.0);
    let exec = ConnectionModel {
        src_node: a,
        src_pin: PinId(4),
        dst_node: b,
        dst_pin: PinId(0),
    };
    let data = ConnectionModel {
        src_pin: PinId(5),
        dst_pin: PinId(1),
        ..exec
    };
    apply(&mut actor, ClientMessage::ConnectExec(exec));
    apply(&mut actor, ClientMessage::ConnectData(data));
    let task = actor.engine.host_state.start_delay(a, 100);
    let trace = NodeTrace {
        node_id: b,
        tick: 0,
        inputs: vec![],
        outputs: vec![],
        fired: vec![],
    };
    actor.traces.insert(b, (trace, Instant::now()));
    apply(
        &mut actor,
        ClientMessage::Batch(vec![
            ClientMessage::SetInputValue {
                id: b,
                pin: PinId(1),
                value: Some(DataValue::from(8.0)),
            },
            ClientMessage::MoveNode {
                id: a,
                position: actor_position(50.0),
            },
        ]),
    );
    let events = apply(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.positions[&a], actor_position(1.0));
    assert!(actor.engine.host_state.active_tasks.contains_key(&task));
    assert!(!actor.traces.contains_key(&b));
    assert!(matches!(events.as_slice(), [
        ServerMessage::NodeUpdated(n), ServerMessage::NodeMoved { id: moved, .. }, ServerMessage::History(_),
    ] if n.model.id == b && *moved == a));
    assert_eq!(actor.engine.exec_connections(), &[exec]);
    assert_eq!(actor.engine.data_connections(), &[data]);
}
