use super::*;

#[test]
fn reconnects_with_backoff_and_ignores_stale_sockets() {
    let mut rig = Rig::new();
    rig.watch(1, "light.a");
    rig.connect();
    let old = rig.ws;
    rig.tick(vec![WsEvent::Closed(old, "gone".into())]);
    assert!(matches!(
        rig.hub.connection_state(),
        Some(ConnectionState::Reconnecting { .. })
    ));
    assert!(
        rig.env
            .logs
            .iter()
            .any(|l| l.starts_with("warn: Not connected to Home Assistant"))
    );
    // Not before the backoff has passed
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 1);
    rig.now += Duration::from_secs(3);
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 2);
    // A late event of the old socket is ignored
    rig.tick(vec![WsEvent::Opened(old)]);
    rig.ws = rig.env.next_ws;
    let new = rig.ws;
    rig.tick(vec![WsEvent::Opened(new)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    assert_eq!(rig.env.sent.remove(0)["type"], "auth");
    rig.recv(vec![json!({"type": "auth_ok"})]);
    rig.serve();
    assert_eq!(rig.hub.connection_state(), Some(ConnectionState::Connected));
    // The subscription is restored and works again
    let change = rig.change("light.a", "off", "on");
    assert_eq!(rig.recv(vec![change]).len(), 1);
}

#[test]
fn failed_connect_is_retried_and_a_bad_token_stops() {
    let mut rig = Rig::new();
    rig.env.fail_connect = true;
    rig.tick(vec![]);
    assert!(
        rig.env
            .logs
            .iter()
            .any(|l| l.contains("Could not open the Home Assistant WebSocket: refused"))
    );
    rig.env.fail_connect = false;
    rig.now += Duration::from_secs(3);
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 1);
    rig.ws = rig.env.next_ws;
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Opened(ws)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    rig.recv(vec![
        json!({"type": "auth_invalid", "message": "Invalid access token"}),
    ]);
    rig.tick(vec![]);
    assert!(matches!(
        rig.hub.connection_state(),
        Some(ConnectionState::AuthFailed(_))
    ));
    assert!(
        rig.env
            .logs
            .iter()
            .any(|l| l.starts_with("error: Home Assistant rejected the access token"))
    );
    assert!(rig.env.closed.contains(&ws), "the socket is dropped");
    rig.now += Duration::from_secs(3600);
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 1, "never retried");
}

#[test]
fn reconfiguring_closes_the_old_socket_and_resubscribes() {
    let mut rig = Rig::new();
    rig.watch(1, "light.a");
    rig.connect();
    let old = rig.ws;
    rig.hub
        .configure("ws://other/api/websocket", "tok2", rig.now, &mut rig.env);
    assert!(rig.env.closed.contains(&old));
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.last().unwrap(), "ws://other/api/websocket");
    // The old socket's events no longer matter
    rig.tick(vec![WsEvent::Message(
        old,
        json!({"type": "auth_required"}).to_string(),
    )]);
    assert!(rig.env.sent.is_empty());
    rig.ws = rig.env.next_ws;
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Opened(ws)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    assert_eq!(
        rig.env.sent.remove(0),
        json!({"type": "auth", "access_token": "tok2"})
    );
    rig.recv(vec![json!({"type": "auth_ok"})]);
    rig.serve();
    let change = rig.change("light.a", "off", "on");
    assert_eq!(rig.recv(vec![change]).len(), 1);
}

#[test]
fn changing_the_entity_and_dropping_the_node_unsubscribe() {
    let mut rig = Rig::new();
    let (a, serial) = rig.watch(1, "light.a");
    rig.connect();
    let first_trigger = rig.triggers["light.a"];

    rig.hub
        .set_entity(a, serial, "light.b", rig.now, &mut rig.env);
    // The old subscription is cancelled and a new one is made
    let sent = rig.env.sent.clone();
    assert!(
        sent.iter()
            .any(|m| m["type"] == "unsubscribe_events" && m["subscription"] == first_trigger),
        "{sent:?}"
    );
    assert!(
        sent.iter()
            .any(|m| m["type"] == "subscribe_trigger" && m["trigger"]["entity_id"] == "light.b"),
        "{sent:?}"
    );
    rig.serve();
    let stale = rig.change("light.a", "off", "on");
    assert!(
        rig.recv(vec![stale]).is_empty(),
        "the old entity no longer reaches the node"
    );
    let change = rig.change("light.b", "off", "on");
    assert_eq!(rig.recv(vec![change]).len(), 1);

    // A stale serial cannot remove the node
    // (Home Assistant's late event for the cancelled subscription makes the session unsubscribe once more)
    rig.env.sent.clear();
    rig.hub.remove_watch(a, serial + 100, rig.now, &mut rig.env);
    assert!(rig.env.sent.is_empty());
    let second_trigger = rig.triggers["light.b"];
    rig.hub.remove_watch(a, serial, rig.now, &mut rig.env);
    assert!(
        rig.env
            .sent
            .iter()
            .any(|m| m["type"] == "unsubscribe_events" && m["subscription"] == second_trigger)
    );
    rig.serve();
    let late = rig.change("light.b", "on", "off");
    assert!(rig.recv(vec![late]).is_empty());
}

#[test]
fn a_failed_subscription_is_logged_and_retried() {
    let mut rig = Rig::new();
    rig.watch(1, "light.ghost");
    rig.tick(vec![]);
    rig.ws = rig.env.next_ws;
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Opened(ws)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    rig.env.sent.clear();
    rig.recv(vec![json!({"type": "auth_ok"})]);
    // Home Assistant refuses the subscription
    let id = rig.env.sent[0]["id"].as_u64().unwrap();
    rig.env.sent.clear();
    rig.recv(vec![json!({"id": id, "type": "result", "success": false, "error": {"code": "invalid_format", "message": "bad"}})]);
    assert!(
        rig.env.logs.iter().any(
            |l| l.starts_with("error: Home Assistant rejected the subscription to light.ghost")
        ),
        "{:?}",
        rig.env.logs
    );
    // After the delay the node subscribes again
    assert!(rig.env.sent.is_empty());
    rig.now += RESUBSCRIBE_DELAY + Duration::from_secs(1);
    rig.tick(vec![]);
    rig.tick(vec![]);
    assert!(
        rig.env
            .sent
            .iter()
            .any(|m| m["type"] == "subscribe_trigger"),
        "{:?}",
        rig.env.sent
    );
}
