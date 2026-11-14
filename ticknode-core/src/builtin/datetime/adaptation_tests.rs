use crate::*;
use ticknode_shared::*;

fn add(engine: &mut TickEngine, name: &str, properties: &[(String, DataValue)]) -> NodeId {
    let id = NodeId::generate();
    engine.create_node(id, name, properties).unwrap();
    id
}

#[test]
fn adaptation_and_graph_roundtrip() {
    let mut engine = TickEngine::new();
    let diff = add(&mut engine, "time/difference", &[]);
    let now = add(&mut engine, "time/now", &[]);

    // Connecting Time of day (Pin 1) to A (Pin 0) adapts time/difference to Time
    engine.connect_data(now, PinId(1), diff, PinId(0)).unwrap();

    let def = engine.node_definition(diff).unwrap();
    assert_eq!(def.pins[0].pin_type, PinType::Data(DataType::Time));
    assert_eq!(def.pins[1].pin_type, PinType::Data(DataType::Time));

    let exported = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_graph(&exported);

    let restored_def = restored.node_definition(diff).unwrap();
    assert_eq!(restored_def.pins[0].pin_type, PinType::Data(DataType::Time));
    assert_eq!(restored.export_graph(), exported);
}

#[test]
fn shift_time_adaptation() {
    let mut engine = TickEngine::new();
    let shift = add(&mut engine, "time/shift", &[]);
    let now = add(&mut engine, "time/now", &[]);

    // Connecting Time of day (Pin 1) to Value (Pin 0) adapts time/shift to Time
    engine.connect_data(now, PinId(1), shift, PinId(0)).unwrap();

    let def = engine.node_definition(shift).unwrap();
    assert_eq!(def.pins[0].pin_type, PinType::Data(DataType::Time));
    assert_eq!(def.pins[2].pin_type, PinType::Data(DataType::Time));

    let exported = engine.export_graph();
    let mut restored = TickEngine::new();
    restored.load_graph(&exported);

    let restored_def = restored.node_definition(shift).unwrap();
    assert_eq!(restored_def.pins[0].pin_type, PinType::Data(DataType::Time));
    assert_eq!(restored.export_graph(), exported);
}

#[test]
fn datetime_nodes_properties_and_options_in_both_locales() {
    let [(_, en), (_, uk)] = translations::CORE_TRANSLATIONS;
    for (locale, source) in [("en", en), ("uk", uk)] {
        for node in [
            "node-time-shift-prop-operation-title",
            "node-time-shift-prop-operation-description",
            "node-time-shift-prop-operation-option-add",
            "node-time-shift-prop-operation-option-subtract",
            "node-time-start-of-prop-unit-title",
            "node-time-start-of-prop-unit-description",
            "node-time-start-of-prop-unit-option-minute",
            "node-time-start-of-prop-unit-option-hour",
            "node-time-start-of-prop-unit-option-day",
            "node-time-start-of-prop-unit-option-week",
            "node-time-start-of-prop-unit-option-month",
            "node-time-start-of-prop-unit-option-year",
            "error-time-shift-overflow",
        ] {
            assert!(
                source.lines().any(|l| l.starts_with(node)),
                "{locale} missing {node}"
            );
        }
    }
}
