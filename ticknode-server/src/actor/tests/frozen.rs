use super::*;
use ticknode_shared::{DetachedConnection, DetachedEnd, GraphModel};

fn frozen_actor(dir: &std::path::Path) -> (Actor, NodeId, DetachedConnection) {
    let mut actor = test_actor(dir);
    let sheet_id = actor.sheets[0].id;
    let source = NodeId::generate();
    let id = NodeId::generate();
    actor.engine.create_node(source, "const", &[]).unwrap();
    actor
        .engine
        .create_node(
            id,
            "math/expression",
            &[("expression".into(), "a + b".into())],
        )
        .unwrap();
    actor.node_sheet.insert(source, sheet_id);
    actor.node_sheet.insert(id, sheet_id);
    actor
        .engine
        .connect_data(source, PinId(0), id, PinId(2))
        .unwrap();
    let mut graph = actor.engine.export_graph();
    graph
        .nodes
        .iter_mut()
        .find(|n| n.id == id)
        .unwrap()
        .properties = vec![("expression".into(), "a".into())];
    assert!(actor.engine.load_graph(&graph).is_empty());
    let d = actor.engine.detached_connections()[0];
    assert_eq!(d.end, DetachedEnd::Target);
    assert_eq!(d.row, 1);
    (actor, id, d)
}

fn edit(actor: &mut Actor, command: ClientMessage) -> Vec<ServerMessage> {
    test_apply(actor, command)
}

#[test]
fn confirm_and_remove_are_undoable_and_noops_leave_history_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, id, d) = frozen_actor(dir.path());
    let frozen = actor.graph_snapshot();
    let events = edit(&mut actor, ClientMessage::ConfirmPins(id));
    assert!(events.iter().any(
        |e| matches!(e, ServerMessage::NodeUpdated(n) if n.model.id == id && n.frozen.is_empty())
    ));
    assert_eq!(actor.engine.detached_connections(), &[d]);
    let confirmed = actor.graph_snapshot();
    assert!(edit(&mut actor, ClientMessage::ConfirmPins(id)).is_empty());
    let events = edit(&mut actor, ClientMessage::Undo);
    assert!(events.iter().any(
        |e| matches!(e, ServerMessage::NodeUpdated(n) if n.model.id == id && !n.frozen.is_empty())
    ));
    assert_eq!(actor.graph_snapshot(), frozen);
    edit(&mut actor, ClientMessage::Redo);
    assert_eq!(actor.graph_snapshot(), confirmed);
    let events = edit(&mut actor, ClientMessage::RemoveDetached(d));
    assert!(events.contains(&ServerMessage::DetachedRemoved(d)));
    assert!(edit(&mut actor, ClientMessage::RemoveDetached(d)).is_empty());
    assert!(edit(&mut actor, ClientMessage::Undo).contains(&ServerMessage::Detached(d)));
    assert_eq!(actor.graph_snapshot(), confirmed);
    assert!(edit(&mut actor, ClientMessage::Redo).contains(&ServerMessage::DetachedRemoved(d)));
    let (reply, _) = mpsc::unbounded_channel();
    assert_eq!(
        actor
            .apply(
                ClientMessage::ConfirmPins(NodeId::generate()),
                actor.sheets.first().map(|s| s.id),
                &reply
            )
            .unwrap_err()
            .key,
        "error-node-not-found"
    );
}

#[test]
fn batch_init_and_postcard_carry_frozen_nodes_and_detached_wires() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, id, d) = frozen_actor(dir.path());
    let snapshot = actor.snapshot(Some(actor.sheets[0].id));
    assert_eq!(
        postcard::from_bytes::<ServerMessage>(&encode(&snapshot)).unwrap(),
        snapshot
    );
    let ServerMessage::Init {
        nodes,
        detached_connections,
        ..
    } = snapshot
    else {
        panic!()
    };
    assert_eq!(detached_connections, vec![d]);
    assert!(
        !nodes
            .iter()
            .find(|n| n.model.id == id)
            .unwrap()
            .frozen
            .is_empty()
    );
    let before = actor.graph_snapshot();
    let command = ClientMessage::Batch(vec![
        ClientMessage::ConfirmPins(id),
        ClientMessage::RemoveDetached(d),
    ]);
    assert_eq!(
        postcard::from_bytes::<ClientMessage>(&postcard::to_allocvec(&command).unwrap()).unwrap(),
        command
    );
    edit(&mut actor, command);
    assert!(actor.engine.frozen_changes(id).is_empty());
    assert!(actor.engine.detached_connections().is_empty());
    edit(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(
        !actor
            .histories
            .get(&actor.sheets[0].id)
            .map_or_default(|h| h.state())
            .can_undo
    );
}

#[test]
fn paste_remaps_detached_wires_and_history_restores_them() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, id, d) = frozen_actor(dir.path());
    let snippet = actor.graph_snapshot();
    let before = snippet.clone();
    let (reply, _) = mpsc::unbounded_channel();
    let (events, created) = actor
        .apply(
            ClientMessage::Paste(snippet),
            actor.sheets.first().map(|s| s.id),
            &reply,
        )
        .unwrap();
    let events: Vec<ServerMessage> = events.into_iter().map(|(_, m)| m).collect();
    let ids = created.unwrap();
    let pasted_id = ids
        .iter()
        .copied()
        .find(|new| actor.engine.node_model(*new).unwrap().type_name == "math/expression")
        .unwrap();
    let pasted = actor.engine.export_snippet(&ids);
    let copied = pasted.detached_connections[0];
    assert_ne!(copied.connection.dst_node, id);
    assert_eq!(copied.connection.dst_node, pasted_id);
    assert_eq!(copied.row, d.row);
    assert!(events.contains(&ServerMessage::Detached(copied)));
    assert!(events.iter().any(|e| matches!(e, ServerMessage::NodeAdded(n) if n.model.id == pasted_id && !n.frozen.is_empty())));
    edit(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(edit(&mut actor, ClientMessage::Redo).contains(&ServerMessage::Detached(copied)));
    assert_eq!(actor.engine.export_snippet(&ids), pasted);
}

#[test]
fn storage_roundtrip_is_stable_and_loads_legacy_json() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, id, d) = frozen_actor(dir.path());
    let other = DetachedConnection {
        exec: true,
        end: DetachedEnd::Source,
        row: 3,
        ..d
    };
    let mut unsorted = actor.graph_snapshot();
    unsorted.detached_connections = vec![other, d];
    assert!(actor.engine.load_graph(&unsorted).is_empty());
    actor.positions.insert(id, Position { x: 12.0, y: 64.0 });
    actor.dirty_since = Some(Instant::now());
    actor.save();
    let sheet_id = actor.sheets[0].id;
    let path = actor.storage.sheet_path(sheet_id);
    let first = std::fs::read_to_string(&path).unwrap();
    let graph: GraphModel = serde_json::from_str(&first).unwrap();
    assert!(graph.nodes.iter().all(|n| n.pins.is_some()));
    assert_eq!(graph.detached_connections, vec![d, other]);
    let mut restored = TickEngine::new();
    let _loaded = actor.storage.load(&mut restored).unwrap();
    assert!(!restored.frozen_changes(id).is_empty());
    actor.storage.save_sheet(sheet_id, &graph).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), first);
    let mut legacy = serde_json::to_value(&graph).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("detached_connections");
    for n in legacy["nodes"].as_array_mut().unwrap() {
        n.as_object_mut().unwrap().remove("pins");
    }
    std::fs::write(&path, serde_json::to_string(&legacy).unwrap()).unwrap();
    actor.storage.load(&mut restored).unwrap();
    assert!(restored.frozen_changes(id).is_empty());
    assert!(restored.detached_connections().is_empty());
}

#[test]
fn settings_paste_accepts_the_new_contract_and_undo_restores_the_freeze() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, id, d) = frozen_actor(dir.path());
    let before = actor.graph_snapshot();
    let events = edit(
        &mut actor,
        ClientMessage::PasteSettings {
            id,
            type_name: "math/expression".into(),
            properties: vec![("expression".into(), "a + c".into())],
        },
    );
    assert!(events.iter().any(
        |e| matches!(e, ServerMessage::NodeUpdated(n) if n.model.id == id && n.frozen.is_empty())
    ));
    assert_eq!(actor.engine.detached_connections(), &[d]);
    edit(&mut actor, ClientMessage::Undo);
    assert_eq!(actor.graph_snapshot(), before);
    assert!(!actor.engine.frozen_changes(id).is_empty());
}

#[test]
fn plugin_enable_broadcasts_detached_instead_of_disconnected() {
    let dir = tempfile::tempdir().unwrap();
    let (mut actor, _, _) = frozen_actor(dir.path());
    let wasm = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    std::fs::create_dir_all(dir.path().join("plugins")).unwrap();
    std::fs::copy(wasm, dir.path().join("plugins/test.wasm")).unwrap();
    actor.plugins.load_all(&mut actor.engine);
    let source = NodeId::generate();
    let target = NodeId::generate();
    actor
        .engine
        .create_node(source, "test/counter", &[])
        .unwrap();
    actor.engine.create_node(target, "test/debug", &[]).unwrap();
    actor.node_sheet.insert(source, actor.sheets[0].id);
    actor.node_sheet.insert(target, actor.sheets[0].id);
    edit(
        &mut actor,
        ClientMessage::SetPluginEnabled {
            name: "test_plugin".into(),
            enabled: false,
        },
    );
    let mut graph = actor.graph_snapshot();
    let pins = graph
        .nodes
        .iter_mut()
        .find(|n| n.id == source)
        .unwrap()
        .pins
        .as_mut()
        .unwrap();
    let row = pins
        .iter()
        .filter(|p| p.direction == PinDirection::Output)
        .count() as u32;
    pins.push(PinSignature {
        id: PinId(99),
        direction: PinDirection::Output,
        kind: PinKind::Exec,
        required: false,
        name: "Old output".into(),
    });
    let c = ConnectionModel {
        src_node: source,
        src_pin: PinId(99),
        dst_node: target,
        dst_pin: PinId(0),
    };
    graph.exec_connections.push(c);
    assert!(actor.engine.load_graph(&graph).is_empty());
    let events = edit(
        &mut actor,
        ClientMessage::SetPluginEnabled {
            name: "test_plugin".into(),
            enabled: true,
        },
    );
    let d = DetachedConnection {
        exec: true,
        connection: c,
        end: DetachedEnd::Source,
        row,
    };
    assert!(events.contains(&ServerMessage::Detached(d)));
    assert!(!events.contains(&ServerMessage::ExecDisconnected(c)));
    assert!(events.iter().any(|e| matches!(e, ServerMessage::NodeUpdated(n) if n.model.id == source && !n.frozen.is_empty())));
    assert!(actor.dirty_since.is_some());
}
