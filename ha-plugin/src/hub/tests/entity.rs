use super::*;

#[test]
fn state_change_reaches_the_node() {
    let mut rig = Rig::new();
    let (a, _) = rig.watch(1, "light.a");
    assert!(
        rig.connect()
            .iter()
            .all(|e| !has_exec(&e.outputs, PinId(0))),
        "baselines do not fire"
    );
    assert_eq!(rig.hub.connection_state(), Some(ConnectionState::Connected));
    assert_eq!(rig.env.connects, [URL]);

    let change = rig.change("light.a", "off", "on");
    let emissions = rig.recv(vec![change]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].node, a);
    let outputs = &emissions[0].outputs;
    assert!(has_exec(outputs, PinId(0)));
    assert_eq!(
        ticknode_plugin_sdk::field_string(data(outputs, PinId(5)), "state").unwrap(),
        "on"
    );
    assert_eq!(data(outputs, PinId(2)), &DataValue::from("off"));
    assert_eq!(
        data(outputs, PinId(1)).get_type(),
        ticknode_plugin_sdk::DataType::String
    );
    // Nothing more is delivered
    assert!(rig.recv(vec![]).is_empty());
}

#[test]
fn entity_changes_queue_across_ticks_in_order() {
    let mut rig = Rig::new();
    let (every, _) = rig.watch(1, "light.a");
    let (other, _) = rig.watch(2, "light.b");
    rig.connect();
    let changes = vec![
        rig.change("light.a", "off", "on"),
        rig.change("light.a", "on", "off"),
        rig.change("light.a", "off", "on"),
        rig.change("light.b", "off", "on"),
        rig.change("light.b", "on", "off"),
        rig.change("light.b", "off", "on"),
    ];
    let first = rig.recv(changes);
    // One emission per node per tick
    assert_eq!(first.len(), 2);
    let of = |emissions: &[Emission], node: NodeId| {
        emissions.iter().find(|e| e.node == node).map(|e| {
            ticknode_plugin_sdk::field_string(data(&e.outputs, PinId(5)), "state")
                .unwrap()
                .to_string()
        })
    };
    // Every event is queued, including changes drained together.
    let second = rig.recv(vec![]);
    let third = rig.recv(vec![]);
    let fourth = rig.recv(vec![]);
    let every_states: Vec<_> = [&first, &second, &third, &fourth]
        .iter()
        .filter_map(|e| of(e, every))
        .collect();
    assert_eq!(every_states, ["on", "off", "on"]);
    let other_states: Vec<_> = [&first, &second, &third, &fourth]
        .iter()
        .filter_map(|e| of(e, other))
        .collect();
    assert_eq!(other_states, ["on", "off", "on"]);
    assert_eq!(of(&first, other), Some("on".into()));
}

#[test]
fn entity_data_has_an_initial_baseline_changes_and_a_reconnected_baseline() {
    let mut rig = Rig::new();
    let id = node(20);
    let serial = rig.hub.add_entity(id);
    rig.hub.set_entity_options(
        id,
        serial,
        crate::entity::ValueType::Bool,
        crate::entity::FireOn::State,
    );
    rig.hub
        .set_entity(id, serial, "light.a", rig.now, &mut rig.env);
    let emissions = rig.connect();
    assert_eq!(emissions.len(), 1);
    assert_eq!(
        data(&emissions[0].outputs, PinId(1)),
        &DataValue::Bool(false)
    );
    assert!(
        !emissions[0]
            .outputs
            .iter()
            .any(|o| matches!(o, Output::Data(PinId(2), _) | Output::Clear(PinId(2))))
    );
    assert!(
        emissions[0]
            .outputs
            .iter()
            .all(|o| matches!(o, Output::Data(..) | Output::Clear(..)))
    );

    let change = rig.change("light.a", "off", "on");
    let emissions = rig.recv(vec![change]);
    assert_eq!(
        data(&emissions[0].outputs, PinId(1)),
        &DataValue::Bool(true)
    );
    assert_eq!(
        data(&emissions[0].outputs, PinId(2)),
        &DataValue::Bool(false)
    );
    assert!(has_exec(&emissions[0].outputs, PinId(0)));
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Closed(ws, "gone".into())]);
    // The scripted fake's current baseline is off. Reconnect must publish it again.
    rig.now += Duration::from_secs(3);
    let emissions = rig.connect();
    assert!(emissions.iter().any(|e| {
        e.node == id
            && e.outputs
                .iter()
                .any(|o| matches!(o,Output::Data(p,v) if p.0 == 1 && *v == DataValue::Bool(false)))
    }));
    assert!(
        emissions
            .iter()
            .filter(|e| e.node == id)
            .flat_map(|e| &e.outputs)
            .all(|o| !matches!(
                o,
                Output::Data(PinId(2), _) | Output::Clear(PinId(2)) | Output::Exec(_)
            ))
    );
    rig.hub.remove_watch(id, serial, rig.now, &mut rig.env);
    assert!(!rig.hub.entities.contains_key(&id));
}

#[test]
fn entity_settings_control_typed_emissions_and_attribute_only_pulses() {
    use crate::entity::{FireOn, ValueType};
    let mut rig = Rig::new();
    let (id, serial) = rig.watch(55, "sensor.voltage");
    rig.hub
        .set_entity_options(id, serial, ValueType::Number, FireOn::State);
    rig.connect();
    let mut event = rig.change("sensor.voltage", "230", "230");
    event["event"]["variables"]["trigger"]["to_state"]["attributes"]["custom"] = json!(42);
    let emissions = rig.recv(vec![event.clone()]);
    assert_eq!(emissions.len(), 1);
    let out = &emissions[0].outputs;
    assert_eq!(data(out, PinId(1)), &DataValue::from(230.0));
    assert_eq!(data(out, PinId(2)), &DataValue::from(230.0));
    assert_eq!(
        ticknode_plugin_sdk::field_number(data(out, PinId(4)), "custom"),
        Some(42.0)
    );
    assert!(!has_exec(out, PinId(0)));
    rig.hub
        .set_entity_options(id, serial, ValueType::Number, FireOn::Any);
    event["event"]["variables"]["trigger"]["to_state"]["attributes"]["custom"] = json!(43);
    event["event"]["variables"]["trigger"]["to_state"]["last_updated"] =
        json!("2026-10-28T14:45:10Z");
    let emissions = rig.recv(vec![event]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, PinId(0)));
    rig.hub
        .set_entity_options(id, serial, ValueType::Text, FireOn::State);
    let event = rig.change("sensor.voltage", "230", "unavailable");
    let emissions = rig.recv(vec![event]);
    assert!(emissions[0].outputs.contains(&Output::Clear(PinId(1))));
    assert_eq!(
        data(&emissions[0].outputs, PinId(2)),
        &DataValue::from("230")
    );
    assert!(has_exec(&emissions[0].outputs, PinId(0)));
}
