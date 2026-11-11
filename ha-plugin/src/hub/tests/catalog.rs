use super::*;

#[test]
fn entity_options_are_published_after_connecting_and_refreshed() {
    let mut rig = Rig::new();
    rig.states = vec![
        ha_state("light.kitchen", "on"),
        json!({"entity_id": "binary_sensor.door", "state": "on", "attributes": {"friendly_name": "Door", "device_class": "door"},
            "last_changed": "2026-10-28T14:45:09+00:00", "last_updated": "2026-10-28T14:45:09+00:00", "context": {"id": "01X"}}),
    ];
    // Nothing is asked for before the connection is up
    rig.tick(vec![]);
    assert_eq!(rig.get_states_requests, 0);
    rig.connect();
    assert_eq!(rig.get_states_requests, 1);
    let options = &rig.env.options[ENTITIES_SOURCE];
    assert_eq!(
        options
            .iter()
            .map(|o| (o.value.as_str(), o.label.as_str(), o.icon.as_deref()))
            .collect::<Vec<_>>(),
        [
            ("light.kitchen", "Lamp", Some("mdi:lightbulb")),
            ("binary_sensor.door", "Door", Some("mdi:door-open")),
        ]
    );
    assert_eq!(options[0].detail, "light.kitchen");

    // Not asked again before the refresh interval has passed
    rig.advance(Duration::from_secs(60));
    assert_eq!(rig.get_states_requests, 1);
    // Then refreshed with the new list
    rig.states.push(ha_state("switch.new", "off"));
    rig.advance(ENTITIES_REFRESH);
    assert_eq!(rig.get_states_requests, 2);
    assert_eq!(rig.env.options[ENTITIES_SOURCE].len(), 3);
}

#[test]
fn entity_options_are_published_again_after_a_reconnect() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on")];
    rig.connect();
    assert_eq!(rig.env.option_calls, 1);
    let old = rig.ws;
    rig.tick(vec![WsEvent::Closed(old, "gone".into())]);
    rig.states.push(ha_state("light.b", "on"));
    rig.now += Duration::from_secs(3);
    rig.tick(vec![]);
    rig.ws = rig.env.next_ws;
    let new = rig.ws;
    rig.tick(vec![WsEvent::Opened(new)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    rig.env.sent.remove(0);
    rig.recv(vec![json!({"type": "auth_ok"})]);
    rig.serve();
    assert_eq!(rig.get_states_requests, 2);
    assert_eq!(rig.env.options[ENTITIES_SOURCE].len(), 2);
}

#[test]
fn a_failed_entity_list_request_is_retried_later() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.a", "on")];
    rig.connect();
    let calls = rig.env.option_calls;
    // The request fails: nothing is published, it is repeated after the retry delay
    let ws = rig.ws;
    // Keep the connection alive, but do not answer `get_states` itself
    rig.advance(ENTITIES_REFRESH - Duration::from_secs(20));
    rig.now += Duration::from_secs(20);
    rig.tick(vec![]);
    let id = rig
        .env
        .sent
        .iter()
        .find(|m| m["type"] == "get_states")
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    rig.env.sent.clear();
    rig.tick(vec![WsEvent::Message(ws, json!({"id": id, "type": "result", "success": false, "error": {"code": "boom", "message": "no"}}).to_string())]);
    assert!(
        rig.env
            .logs
            .iter()
            .any(|l| l.starts_with("warn: Could not read the list of entities")),
        "{:?}",
        rig.env.logs
    );
    assert_eq!(rig.env.option_calls, calls);
    rig.advance(ENTITIES_RETRY);
    assert_eq!(rig.env.option_calls, calls + 1);
}

#[test]
fn actions_refresh_and_calls_request_response_only_when_supported() {
    use crate::actions::{DONE, FAILED, RESPONSE};
    let mut rig = Rig::new();
    rig.services = json!({"light":{"turn_on":{"name":"Turn on","target":{}}},"weather":{"get_forecasts":{"response":{"optional":false}}}});
    rig.states = vec![ha_state("light.a", "off"), ha_state("switch.a", "off")];
    rig.connect();
    assert_eq!(rig.get_services_requests, 1);
    assert_eq!(
        rig.env.options["actions"]
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["weather.get_forecasts"]
    );
    assert_eq!(
        rig.env.options["action_targets"]
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["light.a"]
    );
    assert_eq!(
        rig.env.options["actions:light.a"]
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["light.turn_on", "light.set_state"]
    );
    assert!(
        rig.hub
            .action(
                node(5),
                "light",
                "turn_on",
                "light.a",
                json!({"brightness":120}),
                false,
                true,
                rig.now,
                &mut rig.env
            )
            .is_none()
    );
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs, DONE));
    assert!(rig.calls[0].get("return_response").is_none());
    assert!(
        rig.hub
            .action(
                node(5),
                "weather",
                "get_forecasts",
                "weather.home",
                json!({}),
                false,
                true,
                rig.now,
                &mut rig.env
            )
            .is_none()
    );
    let emissions = rig.serve();
    assert_eq!(rig.calls[1]["return_response"], true);
    assert_eq!(
        ticknode_plugin_sdk::field_number(data(&emissions[0].outputs, RESPONSE), "answer"),
        Some(42.0)
    );
    rig.advance(ENTITIES_REFRESH);
    assert_eq!(rig.get_services_requests, 2);
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Closed(ws, "gone".into())]);
    let out = rig
        .hub
        .action(
            node(5),
            "light",
            "turn_on",
            "",
            json!({}),
            false,
            true,
            rig.now,
            &mut rig.env,
        )
        .expect("fails promptly");
    assert!(has_exec(&out, FAILED));
    rig.now += Duration::from_secs(3);
    rig.connect();
    assert_eq!(rig.get_services_requests, 3);
}

#[test]
fn action_picker_sources_remain_until_the_last_target_user_is_removed() {
    let mut hub = Hub::default();
    let mut env = Fake::default();
    hub.set_action_target(node(1), "light.hall", &mut env);
    assert!(env.options.is_empty(), "no catalog yet");
    hub.services = crate::actions::services(
        &json!({"light":{"turn_on":{"name":"Turn on","target":{"entity":{"domain":"light"}}}}}),
    );
    hub.set_action_target(node(1), " light.hall ", &mut env);
    assert_eq!(
        env.options["actions:light.hall"]
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["light.turn_on", "light.set_state"]
    );
    hub.set_action_target(node(2), "light.hall", &mut env);
    hub.remove_action_target(node(1), &mut env);
    assert_eq!(env.options["actions:light.hall"].len(), 2);
    hub.remove_action_target(node(2), &mut env);
    assert!(env.options["actions:light.hall"].is_empty());
    hub.remove_action_target(node(2), &mut env);
    hub.set_action_target(node(3), "light.hall", &mut env);
    hub.entity_options.push(crate::entities::entity_option(
        &crate::entities::test_state("light.hall", "on", json!({})),
    ));
    hub.remove_action_target(node(3), &mut env);
    assert_eq!(
        env.options["actions:light.hall"].len(),
        2,
        "catalog still uses the source"
    );
    hub.set_action_target(node(4), "light.hall", &mut env);
    hub.set_action_target(node(4), " ", &mut env);
    assert!(!hub.action_targets.contains_key(&node(4)));
}
