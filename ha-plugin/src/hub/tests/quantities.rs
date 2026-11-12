use super::*;
use crate::entity::{FireOn, ValueType};

#[test]
fn quantity_emissions_clear_mismatched_units_and_warn_once_per_node_until_recovery() {
    let mut rig = Rig::new();
    let (id, serial) = rig.watch(60, "sensor.power");
    rig.hub.set_entity_options(
        id,
        serial,
        ValueType::Quantity("quantity.power"),
        FireOn::State,
    );
    rig.connect();
    rig.env.logs.clear();
    // A valid sample recovers from the fake's unitless initial baseline.
    let sequence = std::cell::Cell::new(0);
    let event = |rig: &Rig, unit: Option<&str>| {
        sequence.set(sequence.get() + 1);
        let mut event = rig.change("sensor.power", "1", "2");
        for side in ["from_state", "to_state"] {
            event["event"]["variables"]["trigger"][side]["attributes"] = json!({"unit_of_measurement":unit, "device_class":"power", "sequence":sequence.get()});
            event["event"]["variables"]["trigger"][side]["last_updated"] =
                json!(format!("2026-10-28T14:46:{:02}Z", sequence.get()));
        }
        event
    };
    let emissions = rig.recv(vec![event(&rig, Some("kW"))]);
    assert_eq!(data(&emissions[0].outputs, PinId(1)), &2000.0.into());
    assert_eq!(data(&emissions[0].outputs, PinId(2)), &1000.0.into());
    for unit in [Some("V"), Some("V"), None] {
        let emissions = rig.recv(vec![event(&rig, unit)]);
        for pin in [1, 2] {
            assert!(emissions[0].outputs.contains(&Output::Clear(PinId(pin))));
        }
    }
    let warnings = |rig: &Rig| {
        rig.env
            .logs
            .iter()
            .filter(|s| s.contains("incompatible unit_of_measurement"))
            .count()
    };
    assert_eq!(warnings(&rig), 1);
    let emissions = rig.recv(vec![event(&rig, Some("W"))]);
    assert_eq!(data(&emissions[0].outputs, PinId(1)), &2.0.into());
    rig.recv(vec![event(&rig, Some("V"))]);
    assert_eq!(warnings(&rig), 2);
    // Another node watching the same entity has its own warning state.
    let other = node(61);
    let serial = rig.hub.add_entity(other);
    rig.hub.set_entity_options(
        other,
        serial,
        ValueType::Quantity("quantity.power"),
        FireOn::State,
    );
    rig.hub
        .set_entity(other, serial, "sensor.power", rig.now, &mut rig.env);
    rig.serve();
    assert_eq!(warnings(&rig), 3);
}
