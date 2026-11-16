use super::super::tracking::{LAZY_TTL, TRACK_GRACE};
use super::*;
use crate::conditions::{Kind, evaluate};
use ticknode_plugin_sdk::{Inputs, PluginNode};

fn zone_condition(rig: &mut Rig, n: u128) -> (NodeId, u64, BTreeMap<String, String>) {
    let owner = node(n);
    let serial = rig.hub.add_condition(owner, rig.now);
    let properties = [
        ("entity_id".into(), "person.me".into()),
        ("zone".into(), "zone.home".into()),
    ]
    .into();
    rig.hub
        .set_condition_entities(owner, serial, Kind::Zone, &properties, rig.now);
    (owner, serial, properties)
}
fn count(rig: &Rig, entity: &str) -> usize {
    rig.trigger_requests
        .iter()
        .filter(|r| r["trigger"]["entity_id"] == entity)
        .count()
}

#[test]
fn conditions_share_entity_nodes_subscriptions_and_release_after_grace() {
    let mut rig = Rig::new();
    let (a, sa, _) = zone_condition(&mut rig, 1);
    let (b, sb, _) = zone_condition(&mut rig, 2);
    let (data, sd) = rig.watch(3, "person.me");
    rig.connect();
    assert_eq!(count(&rig, "person.me"), 1);
    assert_eq!(count(&rig, "zone.home"), 1);
    let person = rig.triggers["person.me"];
    rig.hub.remove_watch(data, sd, rig.now, &mut rig.env);
    rig.hub.remove_condition(a, sa, rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE + Duration::from_secs(1));
    assert!(!rig.unsubscribed.contains(&person));
    rig.hub.remove_condition(b, sb, rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE - Duration::from_secs(1));
    assert!(!rig.unsubscribed.contains(&person));
    rig.advance(Duration::from_secs(1));
    assert!(rig.unsubscribed.contains(&person));
    assert!(rig.hub.live.get("person.me").is_none());
}

#[test]
fn retracking_during_grace_reuses_the_subscription_and_loaded_state() {
    let mut rig = Rig::new();
    rig.hub.track(node(1), ["light.a"], rig.now);
    rig.connect();
    let sub = rig.triggers["light.a"];
    rig.hub.untrack(node(1), rig.now);
    rig.advance(Duration::from_secs(59));
    rig.hub.track(node(2), ["light.a"], rig.now);
    rig.advance(TRACK_GRACE);
    assert_eq!(count(&rig, "light.a"), 1);
    assert!(!rig.unsubscribed.contains(&sub));
    assert_eq!(rig.hub.live.state("light.a").unwrap().state, "off");
}

#[test]
fn condition_baseline_and_trigger_events_include_attributes() {
    let mut rig = Rig::new();
    let (_, _, properties) = zone_condition(&mut rig, 1);
    rig.states = vec![
        ha_state("person.me", "home"),
        ha_state("zone.home", "1"),
        ha_state("sensor.unused", "22"),
    ];
    assert!(evaluate(Kind::Zone, &properties, &rig.hub.live, chrono::Utc::now()).is_err());
    rig.connect();
    assert!(evaluate(Kind::Zone, &properties, &rig.hub.live, chrono::Utc::now()).unwrap());
    assert!(
        rig.hub.live.get("sensor.unused").is_none(),
        "the picker catalog is not a state cache"
    );
    let seen = rig.hub.live.revision("person.me");
    let mut event = rig.change("person.me", "home", "home");
    event["event"]["variables"]["trigger"]["to_state"]["attributes"]["latitude"] = json!(50.0);
    rig.recv(vec![event]);
    assert!(rig.hub.live.revision("person.me") > seen);
    assert_eq!(
        rig.hub
            .live
            .state("person.me")
            .unwrap()
            .attributes
            .get_as::<f64>("latitude"),
        Some(50.0)
    );
    rig.recv(vec![rig.change("person.me", "home", "not_home")]);
    assert!(!evaluate(Kind::Zone, &properties, &rig.hub.live, chrono::Utc::now()).unwrap());
    assert!(
        rig.trigger_requests
            .iter()
            .all(|r| r["trigger"].get("to").is_none())
    );
}

#[test]
fn reconnect_keeps_tracked_states_not_ready_until_a_fresh_baseline() {
    let mut rig = Rig::new();
    zone_condition(&mut rig, 1);
    rig.states = vec![ha_state("person.me", "home")];
    rig.connect();
    let reads = rig.baseline_reads.len();
    rig.tick(vec![WsEvent::Closed(rig.ws, "lost".into())]);
    assert!(rig.hub.live.state("person.me").is_err());
    rig.now += Duration::from_secs(3);
    rig.tick(vec![]);
    rig.ws = rig.env.next_ws;
    rig.tick(vec![WsEvent::Opened(rig.ws)]);
    rig.recv(vec![json!({"type":"auth_required"})]);
    rig.env.sent.remove(0);
    rig.recv(vec![json!({"type":"auth_ok"})]);
    assert!(rig.hub.live.state("person.me").is_err());
    rig.states[0]["state"] = json!("not_home");
    rig.serve();
    assert_eq!(count(&rig, "person.me"), 2);
    assert!(rig.baseline_reads.len() > reads);
    assert_eq!(rig.hub.live.state("person.me").unwrap().state, "not_home");
    assert_eq!(rig.get_states_requests, 2);
}

#[test]
fn runtime_action_targets_are_lazy_do_not_skip_before_baseline_and_expire() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.runtime", "on")];
    rig.connect();
    let mut action = crate::actions::ActionNode::new(node(5));
    action.set_property("action", "light.set_state".into());
    let inputs: Inputs = [(PinId(1), "light.runtime".into()), (PinId(2), true.into())]
        .into_iter()
        .collect();
    assert!(
        action
            .run(&inputs, &mut rig.hub, &mut rig.env, rig.now)
            .is_empty()
    );
    assert!(rig.hub.live.get("light.runtime").is_none());
    rig.serve();
    assert_eq!(
        rig.calls.len(),
        1,
        "an unloaded runtime target must not be skipped"
    );
    assert_eq!(count(&rig, "light.runtime"), 1);
    let sub = rig.triggers["light.runtime"];
    rig.advance(Duration::from_secs(100));
    // Accessing an already loaded runtime entity extends its owner lifetime.
    assert_eq!(rig.hub.live.state("light.runtime").unwrap().state, "on");
    rig.tick(vec![]);
    rig.serve();
    rig.advance(LAZY_TTL - Duration::from_secs(1));
    assert!(!rig.unsubscribed.contains(&sub));
    rig.advance(Duration::from_secs(1));
    assert!(
        !rig.unsubscribed.contains(&sub),
        "the expired lazy owner also receives grace"
    );
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&sub));
    assert!(rig.hub.live.get("light.runtime").is_none());
}

#[test]
fn state_access_lazily_loads_entities_and_reports_not_loaded() {
    let mut rig = Rig::new();
    rig.connect();
    assert_eq!(
        rig.hub.live.state("person.runtime").unwrap_err(),
        "Entity person.runtime state is not loaded yet"
    );
    rig.tick(vec![]);
    rig.serve();
    assert_eq!(count(&rig, "person.runtime"), 1);
    assert_eq!(rig.hub.live.state("person.runtime").unwrap().state, "off");
}

#[test]
fn condition_property_changes_replace_dependencies_and_ignore_stale_serials() {
    let mut rig = Rig::new();
    let (owner, serial, mut properties) = zone_condition(&mut rig, 1);
    rig.connect();
    let old = rig.triggers["person.me"];
    properties.insert("entity_id".into(), "person.other".into());
    rig.hub
        .set_condition_entities(owner, serial, Kind::Zone, &properties, rig.now);
    rig.tick(vec![]);
    rig.serve();
    assert_eq!(count(&rig, "person.other"), 1);
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&old));
    rig.hub
        .remove_condition(owner, serial + 1, rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE);
    assert!(!rig.unsubscribed.contains(&rig.triggers["person.other"]));
}

#[test]
fn static_action_targets_replace_all_ids_and_release_on_removal() {
    let mut rig = Rig::new();
    rig.hub
        .set_action_target(node(5), "light.a, light.b", rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(count(&rig, "light.a"), 1);
    assert_eq!(count(&rig, "light.b"), 1);
    let a = rig.triggers["light.a"];
    rig.hub
        .set_action_target(node(5), "light.b", rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&a));
    let b = rig.triggers["light.b"];
    rig.hub.remove_action_target(node(5), rig.now, &mut rig.env);
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&b));
}

#[test]
fn a_runtime_access_retains_its_lazy_owner_even_when_another_node_tracks_statically() {
    let mut rig = Rig::new();
    rig.hub.track(node(1), ["light.shared"], rig.now);
    rig.connect();
    rig.hub.action(
        node(5),
        crate::hub::ActionRequest {
            domain: "light",
            service: "turn_on",
            entity: "light.shared",
            data: json!({}),
            response: false,
            skip_unchanged: true,
        },
        rig.now,
        &mut rig.env,
    );
    rig.serve();
    let sub = rig.triggers["light.shared"];
    rig.hub.untrack(node(1), rig.now);
    rig.advance(TRACK_GRACE);
    assert!(!rig.unsubscribed.contains(&sub));
    rig.advance(LAZY_TTL);
    assert!(rig.unsubscribed.contains(&sub));
}

#[test]
fn real_condition_properties_and_drop_register_and_release_dependencies() {
    use crate::conditions::ConditionNode;
    let mut rig = Rig::new();
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    let mut condition = ConditionNode::new(Kind::Zone, node(1));
    condition.set_property("entity_id", "person.me".into());
    condition.set_property("zone", "zone.home".into());
    let mut sun = ConditionNode::new(Kind::Sun, node(2));
    sun.set_property("after", "sunrise".into());
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    rig.connect();
    assert_eq!(count(&rig, "person.me"), 1);
    assert_eq!(count(&rig, "zone.home"), 1);
    assert_eq!(count(&rig, "sun.sun"), 1);
    let person = rig.triggers["person.me"];
    let solar = rig.triggers["sun.sun"];
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    sun.set_property("after", "none".into());
    drop(condition);
    drop(sun);
    crate::with_hub(|hub, _| std::mem::swap(hub, &mut rig.hub));
    // Node lifecycle methods use the real monotonic clock at the boundary.
    rig.now = Instant::now();
    rig.advance(TRACK_GRACE);
    assert!(rig.unsubscribed.contains(&person));
    assert!(rig.unsubscribed.contains(&solar));
}

#[test]
fn events_and_removals_during_a_baseline_are_not_overwritten_by_its_reply() {
    for removed in [false, true] {
        let mut rig = Rig::new();
        rig.connect();
        rig.hub.track(node(1), ["light.race"], rig.now);
        rig.tick(vec![]);
        let request = std::mem::take(&mut rig.env.sent)
            .into_iter()
            .find(|r| r["type"] == "subscribe_trigger")
            .unwrap();
        let trigger = request["id"].as_u64().unwrap();
        rig.triggers.insert("light.race".into(), trigger);
        rig.recv(vec![
            json!({"id":trigger,"type":"result","success":true,"result":null}),
        ]);
        let read = std::mem::take(&mut rig.env.sent)
            .into_iter()
            .find(|r| r["type"] == "subscribe_entities")
            .unwrap();
        let baseline = |id| {
            vec![
                json!({"id":id,"type":"result","success":true,"result":null}),
                json!({"id":id,"type":"event","event":{"a":{"light.race":{
                    "s":"off","a":{},"c":"01X","lc":1.0,"lu":1.0
                }}}}),
            ]
        };
        // The client's internal baseline confirms the trigger; the hub then reads its shared baseline.
        rig.recv(baseline(read["id"].as_u64().unwrap()));
        let read = std::mem::take(&mut rig.env.sent)
            .into_iter()
            .find(|r| r["type"] == "subscribe_entities")
            .unwrap();
        assert!(rig.hub.live.state("light.race").is_err());
        let mut event = rig.change("light.race", "off", "on");
        if removed {
            event["event"]["variables"]["trigger"]["to_state"] = Value::Null;
        }
        rig.recv(vec![event]);
        assert!(
            rig.hub.live.get("light.race").is_none(),
            "not ready until baseline"
        );
        rig.recv(baseline(read["id"].as_u64().unwrap()));
        if removed {
            assert!(rig.hub.live.get("light.race").is_none());
        } else {
            assert_eq!(rig.hub.live.state("light.race").unwrap().state, "on");
        }
    }
}
