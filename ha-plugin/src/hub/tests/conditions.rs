use super::*;

#[test]
fn live_cache_applies_pushes_removals_and_invalidates_on_disconnect() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.test", "off")];
    rig.hub.track(node(99), ["light.test"], rig.now);
    rig.connect();
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "off");
    assert_eq!(rig.hub.live.offset().unwrap().local_minus_utc(), 7200);
    rig.recv(vec![rig.change("light.test", "off", "on")]);
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "on");
    let mut removal = rig.change("light.test", "on", "off");
    removal["event"]["variables"]["trigger"]["to_state"] = Value::Null;
    rig.recv(vec![removal]);
    assert!(rig.hub.live.state("light.test").is_err());
    rig.tick(vec![WsEvent::Closed(rig.ws, "gone".into())]);
    assert!(rig.hub.live.state("light.test").is_err());
    assert!(rig.hub.live.offset().is_err());
}

#[test]
fn template_condition_first_render_errors_timeout_cancellation_and_disconnect() {
    let mut rig = Rig::new();
    rig.connect();
    let node = node(700);
    let serial = rig.hub.add_condition(node, rig.now);
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
