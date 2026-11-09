use super::*;
use ticknode_shared::*;

mod history;
mod settings;
mod wire_suggest;
mod routes;

#[test]
fn split_protocol_round_trip_updates_effective_pins_and_disconnects() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = TickEngine::new();
    engine.register_types(&[TypeDefinition::new_struct("test.point", "Point", vec![
        FieldDefinition::new("x", "X", DataType::Number),
    ])], &[]).unwrap();
    let source = NodeId::generate();
    let sink = NodeId::generate();
    engine.create_node(source, "type/make/test.point", &[]).unwrap();
    engine.create_node(sink, "type/break/test.point", &[]).unwrap();
    engine.connect_data(source, PinId(0), sink, PinId(0)).unwrap();
    let (events, _) = broadcast::channel(16);
    let mut actor = Actor {
        engine, positions: HashMap::new(), routes: HashMap::new(), storage: Storage::new(dir.path().join("graph.json")),
        plugins: Plugins::new(dir.path()), events, dirty_since: None, traces: HashMap::new(), history: Default::default(), telemetry: super::telemetry::Telemetry::new(std::time::Instant::now()),
    };
    for split in [true, false] {
        let msg = ClientMessage::SetPinSplit { node: source, pin: PinId(0), split };
        let decoded = postcard::from_bytes(&postcard::to_allocvec(&msg).unwrap()).unwrap();
        assert_eq!(msg, decoded);
        let (reply, _) = mpsc::unbounded_channel();
        let (events, _) = actor.apply(decoded, &reply).unwrap();
        if split { assert!(events.iter().any(|m| matches!(m, ServerMessage::DataDisconnected { dst_node, .. } if *dst_node == sink))); }
        let event = events.iter().rev().find(|e| matches!(e, ServerMessage::NodeUpdated(_))).unwrap();
        let decoded: ServerMessage = postcard::from_bytes(&encode(event)).unwrap();
        assert_eq!(*event, decoded);
        let ServerMessage::NodeUpdated(instance) = decoded else { panic!("missing NodeUpdated") };
        assert!(instance.definition.pins.iter().any(|p| p.id == if split { split_field_pin(PinId(0), 0).unwrap() } else { PinId(0) } && p.direction == PinDirection::Output));
    }
}
