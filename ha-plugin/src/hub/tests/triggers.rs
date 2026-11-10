use super::*;

#[test]
fn trigger_replaces_subscription_clears_queue_and_unsubscribes_on_removal() {
    use crate::triggers::{INFO, Platform, TRIGGERED};
    let mut rig = Rig::new();
    rig.connect();
    let a = node(500);
    let serial = rig.hub.add_trigger(a, rig.now);
    let config = trigger_config(Platform::Event, &[("event_type", "button_a")]);
    rig.hub
        .set_trigger(a, serial, Some(config.clone()), rig.now, &mut rig.env);
    rig.serve();
    let first = rig.triggers["button_a"];
    assert_eq!(
        rig.trigger_requests.last().unwrap()["trigger"],
        json!({"trigger":"event","event_type":["button_a"]})
    );
    let fired = |id, n| fire(id, json!({"platform":"event","description":n}));
    let emissions = rig.recv(vec![fired(first, 1), fired(first, 2), fired(first, 3)]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].node, a);
    assert!(has_exec(&emissions[0].outputs, TRIGGERED));
    assert_eq!(
        ticknode_plugin_sdk::field(data(&emissions[0].outputs, INFO), "description"),
        Some(&DataValue::from(1i64))
    );
    let emissions = rig.tick(vec![]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(
        ticknode_plugin_sdk::field(data(&emissions[0].outputs, INFO), "description"),
        Some(&DataValue::from(2i64))
    );
    let sent = rig.trigger_requests.len();
    rig.hub
        .set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.serve();
    assert_eq!(
        rig.trigger_requests.len(),
        sent,
        "equivalent config keeps the subscription"
    );
    // Queue another fire, then invalidate the config. No queued old fire may escape.
    rig.recv(vec![fired(first, 4), fired(first, 5)]);
    rig.hub.set_trigger(a, serial, None, rig.now, &mut rig.env);
    assert!(rig.serve().is_empty());
    assert!(rig.unsubscribed.contains(&first));
    assert!(rig.tick(vec![]).is_empty());
    let config = trigger_config(Platform::Event, &[("event_type", "button_b")]);
    rig.hub
        .set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.serve();
    let second = rig.triggers["button_b"];
    assert_ne!(first, second);
    assert!(rig.recv(vec![fired(first, 6)]).is_empty());
    rig.hub.remove_trigger(a, serial + 1, rig.now, &mut rig.env);
    assert!(
        rig.hub.triggers.contains_key(&a),
        "late drop cannot remove another generation"
    );
    rig.hub.remove_trigger(a, serial, rig.now, &mut rig.env);
    rig.serve();
    assert!(rig.unsubscribed.contains(&second));
    assert!(rig.recv(vec![fired(second, 7)]).is_empty());
}

#[test]
fn trigger_rejections_are_node_errors_and_retry_only_after_thirty_seconds() {
    use crate::triggers::Platform;
    let mut rig = Rig::new();
    rig.connect();
    rig.trigger_error = Some("bad template");
    let a = node(501);
    let b = node(502);
    let serial = rig.hub.add_trigger(a, rig.now);
    let other = rig.hub.add_trigger(b, rig.now);
    let config = trigger_config(Platform::Template, &[("value_template", "{{ broken }}")]);
    rig.hub
        .set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.serve();
    assert_eq!(rig.trigger_requests.len(), 1);
    let errors = rig.hub.take_trigger_errors(a, serial);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("bad template"));
    assert!(rig.hub.take_trigger_errors(a, serial).is_empty());
    assert!(rig.hub.take_trigger_errors(b, other).is_empty());
    rig.advance(Duration::from_secs(29));
    assert_eq!(rig.trigger_requests.len(), 1);
    rig.trigger_error = None;
    rig.advance(Duration::from_secs(1));
    assert_eq!(rig.trigger_requests.len(), 2);
    assert_eq!(
        rig.recv(vec![fire(
            rig.triggers["template"],
            json!({"platform":"template"})
        )])
        .len(),
        1
    );
}

#[test]
fn offline_trigger_rejection_retries_and_reconfiguration_restores_it() {
    use crate::triggers::Platform;
    let mut rig = Rig::new();
    let a = node(503);
    let serial = rig.hub.add_trigger(a, rig.now);
    let config = trigger_config(Platform::Event, &[("event_type", "button")]);
    rig.hub
        .set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.trigger_error = Some("not allowed");
    rig.connect();
    let errors = rig.hub.take_trigger_errors(a, serial);
    assert!(
        errors
            .iter()
            .any(|e| e.contains("rejected") && e.contains("not allowed")),
        "{errors:?}"
    );
    rig.trigger_error = None;
    rig.advance(RESUBSCRIBE_DELAY);
    assert_eq!(rig.trigger_requests.len(), 2);
    // Reconfigure the connection: the same node subscribes in the new session.
    rig.hub.configure(URL, "tok", rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(rig.trigger_requests.len(), 3);
    assert_eq!(
        rig.recv(vec![fire(
            rig.triggers["button"],
            json!({"platform":"event"})
        )])
        .len(),
        1
    );
}

#[test]
fn trigger_edits_and_removal_before_the_first_tick_cancel_ready_pending_subscriptions() {
    use crate::triggers::Platform;
    let mut rig = Rig::new();
    let a = node(504);
    let serial = rig.hub.add_trigger(a, rig.now);
    for event in ["off", "on", "unknown"] {
        let config = trigger_config(Platform::Event, &[("event_type", event)]);
        rig.hub
            .set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    }
    let b = node(505);
    let removed = rig.hub.add_trigger(b, rig.now);
    let config = trigger_config(Platform::Event, &[("event_type", "removed")]);
    rig.hub
        .set_trigger(b, removed, Some(config), rig.now, &mut rig.env);
    rig.hub.remove_trigger(b, removed, rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(
        rig.trigger_requests.len(),
        1,
        "only the final config is sent after connecting"
    );
    assert_eq!(
        rig.trigger_requests[0]["trigger"],
        json!({"trigger":"event","event_type":["unknown"]})
    );
}
