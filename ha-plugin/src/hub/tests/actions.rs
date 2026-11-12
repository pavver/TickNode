use super::*;
use crate::actions::{ActionNode, DONE, ERROR, FAILED};
use ticknode_plugin_sdk::{Inputs, PluginNode};

fn call(
    rig: &mut Rig,
    domain: &str,
    service: &str,
    target: &str,
    fields: Value,
    skip: bool,
) -> Vec<Output> {
    if let Some(outputs) = rig.hub.action(
        node(5),
        domain,
        service,
        target,
        fields,
        false,
        skip,
        rig.now,
        &mut rig.env,
    ) {
        return outputs;
    }
    rig.serve()
        .into_iter()
        .find(|emission| emission.node == node(5))
        .expect("action completed")
        .outputs
}

fn cache_event(rig: &Rig, entity: &str, state: &str) -> Value {
    json!({"id":rig.cache_id.unwrap(),"type":"event","event":{
        "event_type":"state_changed","data":{
            "entity_id":entity,"old_state":ha_state(entity,"off"),"new_state":ha_state(entity,state)
        },"origin":"LOCAL","time_fired":"2026-10-28T14:45:09Z","context":{"id":"01X"}
    }})
}

#[test]
fn skipping_fires_done_logs_and_follows_live_changes_and_disconnects() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on")];
    rig.connect();
    let outputs = call(&mut rig, "light", "turn_on", "light.a", json!({}), true);
    assert!(has_exec(&outputs, DONE));
    assert_eq!(data(&outputs, ERROR), &DataValue::from(""));
    assert!(rig.calls.is_empty());
    assert!(
        rig.env
            .logs
            .iter()
            .any(|message| message.contains("Skipped light.turn_on"))
    );
    // The shared state_changed subscription updates the decision immediately.
    rig.recv(vec![cache_event(&rig, "light.a", "off")]);
    assert!(has_exec(
        &call(&mut rig, "light", "turn_on", "light.a", json!({}), true),
        DONE
    ));
    assert_eq!(rig.calls.len(), 1);
    rig.recv(vec![cache_event(&rig, "light.a", "on")]);
    call(&mut rig, "light", "turn_on", "light.a", json!({}), true);
    assert_eq!(rig.calls.len(), 1);
    call(&mut rig, "light", "turn_on", "light.a", json!({}), false);
    assert_eq!(rig.calls.len(), 2);
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Closed(ws, "gone".into())]);
    assert!(has_exec(
        &call(&mut rig, "light", "turn_on", "light.a", json!({}), true),
        FAILED
    ));
}

#[test]
fn unresolved_targets_extra_fields_and_partial_multi_target_matches_still_call_ha() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on"), ha_state("light.b", "on")];
    rig.connect();
    call(
        &mut rig,
        "light",
        "turn_on",
        "light.a,light.b",
        json!({}),
        true,
    );
    assert!(rig.calls.is_empty());
    for (target, fields) in [
        ("light.a", json!({"brightness":120})),
        ("light.missing", json!({})),
        ("light.a,light.missing", json!({})),
        ("", json!({"area_id":"kitchen"})),
        ("", json!({"device_id":"123"})),
        ("", json!({"label_id":"night"})),
    ] {
        let count = rig.calls.len();
        assert!(has_exec(
            &call(&mut rig, "light", "turn_on", target, fields, true),
            DONE
        ));
        assert_eq!(rig.calls.len(), count + 1);
    }
    rig.recv(vec![cache_event(&rig, "light.b", "off")]);
    call(
        &mut rig,
        "light",
        "turn_on",
        "light.a, light.b",
        json!({}),
        true,
    );
    assert_eq!(
        rig.calls.last().unwrap()["target"]["entity_id"],
        json!(["light.a", "light.b"])
    );
    for state in ["unknown", "unavailable"] {
        rig.recv(vec![cache_event(&rig, "light.a", state)]);
        let count = rig.calls.len();
        call(&mut rig, "light", "turn_on", "light.a", json!({}), true);
        assert_eq!(rig.calls.len(), count + 1);
    }
}

#[test]
fn desired_state_maps_both_booleans_and_shares_the_skip_path() {
    let mut rig = Rig::new();
    rig.connect();
    for (domain, positive, negative, state) in [
        ("switch", "turn_on", "turn_off", "on"),
        ("light", "turn_on", "turn_off", "on"),
        ("fan", "turn_on", "turn_off", "on"),
        ("input_boolean", "turn_on", "turn_off", "on"),
        ("siren", "turn_on", "turn_off", "on"),
        ("humidifier", "turn_on", "turn_off", "on"),
        ("automation", "turn_on", "turn_off", "on"),
        ("remote", "turn_on", "turn_off", "on"),
        ("lock", "lock", "unlock", "locked"),
        ("cover", "open_cover", "close_cover", "open"),
    ] {
        let target = format!("{domain}.a");
        rig.hub
            .live
            .snapshot(&[serde_json::from_value(ha_state(&target, state)).unwrap()]);
        let count = rig.calls.len();
        assert!(has_exec(
            &call(
                &mut rig,
                domain,
                "set_state",
                &target,
                json!({"state":true}),
                true
            ),
            DONE
        ));
        assert_eq!(rig.calls.len(), count);
        for (state, service) in [(true, positive), (false, negative)] {
            assert!(has_exec(
                &call(
                    &mut rig,
                    domain,
                    "set_state",
                    &target,
                    json!({"state":state}),
                    false
                ),
                DONE
            ));
            let request = rig.calls.last().unwrap();
            assert_eq!(request["service"], service);
            assert_eq!(request["service_data"], json!({}));
        }
        let negative_state = match domain {
            "lock" => "unlocked",
            "cover" => "closed",
            _ => "off",
        };
        rig.hub
            .live
            .snapshot(&[serde_json::from_value(ha_state(&target, negative_state)).unwrap()]);
        let count = rig.calls.len();
        assert!(has_exec(
            &call(
                &mut rig,
                domain,
                "set_state",
                &target,
                json!({"state":false}),
                true
            ),
            DONE
        ));
        assert_eq!(rig.calls.len(), count);
        let count = rig.calls.len();
        assert!(has_exec(
            &call(&mut rig, domain, "set_state", &target, json!({}), true),
            FAILED
        ));
        assert_eq!(rig.calls.len(), count);
    }
}

#[test]
fn desired_state_node_uses_its_snapshotted_bool_pin() {
    let mut rig = Rig::new();
    rig.connect();
    let mut action = ActionNode::new(node(5));
    action.set_property("target", "lock.door".into());
    action.set_property("action", "lock.set_state".into());
    for (state, service) in [(false, "unlock"), (true, "lock")] {
        let inputs = [(PinId(2), DataValue::Bool(state))].into_iter().collect();
        action.run(&inputs, &mut rig.hub, &mut rig.env, rig.now);
        let emissions = rig.serve();
        assert!(has_exec(&emissions[0].outputs, DONE));
        assert_eq!(rig.calls.last().unwrap()["service"], service);
    }
}

#[test]
fn node_quantity_snapshot_converts_before_calling_or_comparing_in_both_target_modes() {
    let mut rig = Rig::new();
    let mut state = ha_state("number.power", "1.5");
    state["attributes"] = json!({"unit_of_measurement":"kW","device_class":"power"});
    rig.states = vec![state];
    rig.services = json!({"number":{"set_value":{"target":{},"fields":{"value":{"required":true,"selector":{"number":{}}}}}}});
    rig.connect();
    // Use the same setter/snapshot path as the WASM node, with the scripted hub's live catalog.
    crate::with_hub(|hub, _| {
        hub.services = rig.hub.services.clone();
        hub.live.snapshot(&rig.hub.live.all_states());
    });
    let mut action = ActionNode::new(node(5));
    action.set_property("target", "number.power".into());
    action.set_property("action", "number.set_value".into());
    let inputs: Inputs = [(PinId(2), 1500.0.into())].into_iter().collect();
    let outputs = action.run(&inputs, &mut rig.hub, &mut rig.env, rig.now);
    assert!(has_exec(&outputs, DONE));
    assert!(rig.calls.is_empty());
    action.set_property("skip_unchanged", "no".into());
    assert!(
        action
            .run(&inputs, &mut rig.hub, &mut rig.env, rig.now)
            .is_empty()
    );
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs, DONE));
    assert_eq!(rig.calls[0]["service_data"]["value"], 1.5);
    action.set_property("skip_unchanged", "yes".into());
    action.set_property("target", "".into());
    assert!(action.definition().pins.iter().any(|p| p.id == PinId(2)
        && p.pin_type
            == ticknode_shared::PinType::Data(ticknode_shared::DataType::Named(
                "quantity.power".into()
            ))));
    let same: Inputs = [(PinId(1), "number.power".into()), (PinId(2), 1500.0.into())]
        .into_iter()
        .collect();
    // Deliver the state revision confirming the previous call before checking for skips.
    rig.recv(vec![cache_event(&rig, "number.power", "1.5")]);
    // Keep the unit metadata in the cache, as in the original snapshot.
    rig.hub
        .live
        .snapshot(&[serde_json::from_value(rig.states[0].clone()).unwrap()]);
    assert!(has_exec(
        &action.run(&same, &mut rig.hub, &mut rig.env, rig.now),
        DONE
    ));
    assert_eq!(rig.calls.len(), 1);
    let overridden = [(PinId(1), "number.other".into()), (PinId(2), 1500.0.into())]
        .into_iter()
        .collect();
    action.run(&overridden, &mut rig.hub, &mut rig.env, rig.now);
    rig.serve();
    assert_eq!(rig.calls[1]["target"]["entity_id"], json!(["number.other"]));
    let inputs = [(PinId(1), "number.power".into()), (PinId(2), 2000.0.into())]
        .into_iter()
        .collect();
    rig.service_error = Some("rejected");
    action.run(&inputs, &mut rig.hub, &mut rig.env, rig.now);
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs, FAILED));
    assert_eq!(rig.calls[2]["service_data"]["value"], 2.0);
}

#[test]
fn climate_without_an_entity_unit_uses_the_ha_configuration_unit() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("climate.room", "heat")];
    rig.services = json!({"climate":{"set_temperature":{"target":{},"fields":{"temperature":{"selector":{"number":{}}}}}}});
    rig.connect();
    assert_eq!(rig.hub.live.temperature_unit(), Some("°F"));
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    let mut action = ActionNode::new(node(5));
    action.set_property("target", "climate.room".into());
    action.set_property("action", "climate.set_temperature".into());
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    let inputs = [(PinId(2), 20.0.into())].into_iter().collect();
    action.run(&inputs, &mut rig.hub, &mut rig.env, rig.now);
    rig.serve();
    assert!(
        (rig.calls[0]["service_data"]["temperature"]
            .as_f64()
            .unwrap()
            - 68.0)
            .abs()
            < 1e-10
    );
}

#[test]
fn a_call_awaiting_its_state_update_is_never_skipped_against_the_stale_cache() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("switch.psu", "off")];
    rig.connect();
    call(&mut rig, "switch", "turn_on", "switch.psu", json!({}), true);
    assert_eq!(rig.calls.len(), 1);
    // HA has not reported "on" yet: the cache still says "off", so turn_off must still be sent.
    call(
        &mut rig,
        "switch",
        "turn_off",
        "switch.psu",
        json!({}),
        true,
    );
    assert_eq!(rig.calls.len(), 2);
    // Once the state arrives, skipping resumes.
    rig.recv(vec![cache_event(&rig, "switch.psu", "off")]);
    call(
        &mut rig,
        "switch",
        "turn_off",
        "switch.psu",
        json!({}),
        true,
    );
    assert_eq!(rig.calls.len(), 2);
}

#[test]
fn input_target_set_state_uses_bool_snapshot_and_skip_unchanged() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("lock.door", "locked")];
    rig.connect();
    let mut action = ActionNode::new(node(5));
    action.set_property("action", "lock.set_state".into());
    assert_eq!(action.definition().properties[0].input, Some(PinId(1)));
    let same = [(PinId(1), "lock.door".into()), (PinId(2), true.into())]
        .into_iter()
        .collect();
    assert!(has_exec(
        &action.run(&same, &mut rig.hub, &mut rig.env, rig.now),
        DONE
    ));
    assert!(rig.calls.is_empty());
    let changed = [(PinId(1), "lock.door".into()), (PinId(2), false.into())]
        .into_iter()
        .collect();
    action.run(&changed, &mut rig.hub, &mut rig.env, rig.now);
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs, DONE));
    assert_eq!(rig.calls[0]["service"], "unlock");
    assert_eq!(rig.calls[0]["target"]["entity_id"], json!(["lock.door"]));
}
