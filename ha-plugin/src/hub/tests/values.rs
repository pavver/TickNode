use super::super::tracking::TRACK_GRACE;
use super::*;

#[test]
fn engine_values_share_entity_subscription_and_release_only_their_owner() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "off")];
    let (owner, serial) = rig.watch(1, "light.a");
    rig.hub
        .watch_values(vec!["light.a".into()], vec![], rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(
        rig.trigger_requests
            .iter()
            .filter(|r| r["trigger"]["entity_id"] == "light.a")
            .count(),
        1
    );
    assert!(rig.env.values["light.a"].is_some());
    let event = rig.change("light.a", "off", "on");
    rig.recv(vec![event]);
    assert_eq!(
        ticknode_plugin_sdk::field(rig.env.values["light.a"].as_ref().unwrap(), "state"),
        Some(&DataValue::Bool(true))
    );
    let sub = rig.triggers["light.a"];
    rig.hub
        .watch_values(vec![], vec!["light.a".into()], rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE + Duration::from_secs(1));
    assert!(!rig.unsubscribed.contains(&sub));
    rig.hub.remove_watch(owner, serial, rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&sub));
    assert_eq!(rig.get_states_requests, 1);
}

#[test]
fn values_disappear_on_disconnect_and_return_from_shared_baseline() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on")];
    rig.hub
        .watch_values(vec!["light.a".into()], vec![], rig.now, &mut rig.env);
    rig.connect();
    assert!(rig.env.values["light.a"].is_some());
    rig.tick(vec![WsEvent::Closed(rig.ws, "lost".into())]);
    assert_eq!(rig.env.values["light.a"], None);
}

#[test]
fn only_debounced_snapshots_change_catalogs_and_identical_shapes_share_memory() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on"), ha_state("light.b", "off")];
    for state in &mut rig.states {
        state["attributes"]["custom"] = json!(1);
    }
    rig.hub
        .watch_values(vec!["light.a".into()], vec![], rig.now, &mut rig.env);
    rig.connect();
    let original = rig.env.value_catalog.clone();
    let calls = rig.env.value_catalog_calls;
    assert!(std::sync::Arc::ptr_eq(
        &rig.hub.catalog[0].value_fields,
        &rig.hub.catalog[1].value_fields
    ));
    let mut change = rig.change("light.a", "on", "off");
    change["event"]["variables"]["trigger"]["to_state"]["attributes"]["new_attribute"] = json!(42);
    change["event"]["variables"]["trigger"]["to_state"]["attributes"]["custom"] = json!("changed");
    rig.recv(vec![change]);
    assert_eq!(rig.env.value_catalog, original);
    assert_eq!(rig.env.value_catalog_calls, calls);
    let value = rig.env.values["light.a"].as_ref().unwrap();
    assert_eq!(
        ticknode_shared::plugin_values::project(value, "attributes.new_attribute"),
        Some(&42i64.into())
    );

    rig.states[0]["attributes"]["new_attribute"] = json!(42);
    rig.states[0]["attributes"]["custom"] = json!("changed");
    let event = json!({"id":rig.registry_id.unwrap(),"type":"event","event":{
        "event_type":"entity_registry_updated", "data":{"action":"update"},
        "origin":"LOCAL","time_fired":"2026-10-30T10:00:00Z","context":{"id":"t"}}});
    rig.recv(vec![event]);
    rig.advance(Duration::from_secs(9));
    assert_eq!(rig.env.value_catalog_calls, calls);
    rig.advance(Duration::from_secs(1));
    assert_eq!(rig.env.value_catalog_calls, calls + 1);
    assert_eq!(rig.get_states_requests, 2);
    assert!(
        rig.env.value_catalog[0]
            .fields
            .iter()
            .any(|field| field.path == "attributes.new_attribute")
    );
    assert!(!std::sync::Arc::ptr_eq(
        &rig.hub.catalog[0].value_fields,
        &rig.hub.catalog[1].value_fields
    ));
}
