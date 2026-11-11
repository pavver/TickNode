use super::*;
use ticknode_shared::*;

pub(super) fn test_actor(dir: &std::path::Path) -> Actor {
    let (events, _) = broadcast::channel(256);
    let sheet = SheetInfo::new(SheetId::generate(), "Main");
    Actor {
        engine: TickEngine::new(),
        server_settings: ServerSettings::default(),
        sheets: vec![sheet],
        node_sheet: HashMap::new(),
        frame_sheet: HashMap::new(),
        histories: HashMap::new(),
        positions: HashMap::new(),
        sizes: HashMap::new(),
        routes: HashMap::new(),
        frames: BTreeMap::new(),
        storage: Storage::new(dir),
        plugins: Plugins::new(dir),
        events,
        dirty_since: None,
        dirty_sheets: HashSet::new(),
        dirty_sheets_meta: false,
        traces: HashMap::new(),
        telemetry: super::telemetry::Telemetry::new(Instant::now()),
        last_sheets_broadcast: Instant::now(),
        last_sheet_summaries: Vec::new(),
    }
}

pub(super) fn test_apply(actor: &mut Actor, msg: ClientMessage) -> Vec<ServerMessage> {
    let sheet = actor.sheets.first().map(|s| s.id);
    let (reply, _) = mpsc::unbounded_channel();
    actor.apply(msg, sheet, &reply).unwrap().0.into_iter().map(|(_, m)| m).collect()
}

mod history;
mod settings;
mod wire_suggest;
mod routes;
mod frozen;
mod resize;
mod node_errors;
mod frames;
mod sheets;
mod variables;
mod server_settings;

#[test]
fn split_protocol_round_trip_updates_effective_pins_and_disconnects() {
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    let sheet_id = actor.sheets[0].id;
    actor.engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
    ])], &[]).unwrap();
    let source = NodeId::generate();
    let sink = NodeId::generate();
    actor.engine.create_node(source, "type/make/test.point", &[]).unwrap();
    actor.engine.create_node(sink, "type/break/test.point", &[]).unwrap();
    actor.node_sheet.insert(source, sheet_id);
    actor.node_sheet.insert(sink, sheet_id);
    actor.engine.connect_data(source, PinId(0), sink, PinId(0)).unwrap();

    for split in [true, false] {
        let msg = ClientMessage::SetPinSplit { node: source, pin: PinId(0), split };
        let decoded = postcard::from_bytes(&postcard::to_allocvec(&msg).unwrap()).unwrap();
        assert_eq!(msg, decoded);
        let (reply, _) = mpsc::unbounded_channel();
        let (events, _) = actor.apply(decoded, Some(sheet_id), &reply).unwrap();
        let events: Vec<ServerMessage> = events.into_iter().map(|(_, m)| m).collect();
        if split { assert!(events.iter().any(|m| matches!(m, ServerMessage::DataDisconnected { dst_node, .. } if *dst_node == sink))); }
        let event = events.iter().rev().find(|e| matches!(e, ServerMessage::NodeUpdated(_))).unwrap();
        let decoded: ServerMessage = postcard::from_bytes(&encode(event)).unwrap();
        assert_eq!(*event, decoded);
        let ServerMessage::NodeUpdated(instance) = decoded else { panic!("missing NodeUpdated") };
        assert!(instance.definition.pins.iter().any(|p| p.id == if split { split_field_pin(PinId(0), 0).unwrap() } else { PinId(0) } && p.direction == PinDirection::Output));
    }
}
mod property_validation;
mod property_input;

mod plugin_settings;
