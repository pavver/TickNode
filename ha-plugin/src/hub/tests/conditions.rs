use super::*;

#[test]
fn live_cache_applies_pushes_removals_and_invalidates_on_disconnect() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.test", "off")];
    rig.connect();
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "off");
    assert_eq!(rig.hub.live.offset().unwrap().local_minus_utc(), 7200);
    let id = rig.cache_id.unwrap();
    let event = |state: Value| {
        json!({"id":id,"type":"event","event":{"event_type":"state_changed",
        "data":{"entity_id":"light.test","old_state":ha_state("light.test","off"),"new_state":state},
        "origin":"LOCAL","time_fired":"2026-10-30T10:00:00Z","context":{"id":"t"}}})
    };
    rig.recv(vec![event(ha_state("light.test", "on"))]);
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "on");
    // An event racing a get_states response wins over the older snapshot.
    rig.hub.live.begin_snapshot();
    rig.recv(vec![event(ha_state("light.test", "newer"))]);
    rig.hub
        .live
        .snapshot(&[serde_json::from_value(ha_state("light.test", "old")).unwrap()]);
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "newer");
    rig.hub.live.begin_snapshot();
    rig.recv(vec![event(Value::Null)]);
    rig.hub
        .live
        .snapshot(&[serde_json::from_value(ha_state("light.test", "old")).unwrap()]);
    assert!(
        rig.hub.live.state("light.test").is_err(),
        "removal must win over a stale snapshot"
    );
    rig.tick(vec![WsEvent::Closed(rig.ws, "gone".into())]);
    assert!(rig.hub.live.state("light.test").is_err());
    assert!(rig.hub.live.offset().is_err());
}

#[test]
fn template_condition_first_render_errors_timeout_cancellation_and_disconnect() {
    let mut rig = Rig::new();
    rig.connect();
    let node = node(700);
    let serial = rig.hub.add_condition(node);
    let start = |rig: &mut Rig| {
        assert!(
            rig.hub
                .render_condition(node, serial, "{{ test }}", rig.now, &mut rig.env)
                .is_empty()
        );
        rig.serve();
        *rig.render_ids.last().unwrap()
    };
    let id = start(&mut rig);
    let event =
        |id, result| json!({"id":id,"type":"event","event":{"result":result,"listeners":{}}});
    let emissions = rig.recv(vec![event(id, json!(true)), event(id, json!(false))]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::YES));
    assert_eq!(
        data(&emissions[0].outputs, crate::conditions::RESULT),
        &DataValue::Bool(true)
    );
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    let id = start(&mut rig);
    let emissions = rig.recv(vec![
        json!({"id":id,"type":"event","event":{"error":"render failed","level":"ERROR"}}),
    ]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
    let id = start(&mut rig);
    rig.now += Duration::from_secs(31);
    let emissions = rig.tick(vec![]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    let id = start(&mut rig);
    rig.hub
        .cancel_templates(node, serial, rig.now, &mut rig.env);
    assert!(rig.hub.templates.is_empty());
    assert!(rig.recv(vec![event(id, json!(true))]).is_empty());
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    start(&mut rig);
    let emissions = rig.tick(vec![WsEvent::Closed(rig.ws, "lost".into())]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
}
