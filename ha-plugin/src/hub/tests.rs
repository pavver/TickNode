use super::*;
use crate::entities::ENTITIES_SOURCE;
use serde_json::{Value, json};
use ticknode_plugin_sdk::{DataValue, Emission, Output, PinId, WsEvent};
use crate::queue::Mode;
use crate::triggers::outputs as trigger_outputs;
use ticknode_shared::Uuid;

const URL: &str = "ws://ha.test/api/websocket";

#[derive(Default)]
struct Fake {
    next_ws: WsId,
    connects: Vec<String>,
    closed: Vec<WsId>,
    sent: Vec<Value>,
    logs: Vec<String>,
    fail_connect: bool,
    /// Latest options per source and how many times the complete entity list was published.
    options: HashMap<String, Vec<PropertyOption>>,
    option_calls: usize,
}

impl Env for Fake {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String> {
        if self.fail_connect {
            return Err("refused".into());
        }
        self.next_ws += 1;
        self.connects.push(url.to_string());
        Ok(self.next_ws)
    }

    fn ws_send(&mut self, _: WsId, text: &str) -> Result<(), String> {
        self.sent.push(serde_json::from_str(text).unwrap());
        Ok(())
    }

    fn ws_close(&mut self, id: WsId) {
        self.closed.push(id);
    }

    fn info(&mut self, message: &str) {
        self.logs.push(format!("info: {message}"));
    }

    fn warn(&mut self, message: &str) {
        self.logs.push(format!("warn: {message}"));
    }

    fn error(&mut self, message: &str) {
        self.logs.push(format!("error: {message}"));
    }

    fn set_options(&mut self, source: &str, options: &[PropertyOption]) {
        if source == ENTITIES_SOURCE { self.option_calls += 1; }
        self.options.insert(source.to_string(), options.to_vec());
    }
}

fn node(n: u128) -> NodeId {
    NodeId(Uuid::from_u128(n))
}

fn ha_state(entity: &str, state: &str) -> Value {
    json!({
        "entity_id": entity, "state": state, "attributes": {"friendly_name": "Lamp"},
        "last_changed": "2026-10-28T14:45:09.185007+00:00",
        "last_reported": "2026-10-28T14:45:09.185007+00:00",
        "last_updated": "2026-10-28T14:45:09.185007+00:00",
        "context": {"id": "01X", "parent_id": null, "user_id": null}
    })
}

/// A scripted Home Assistant on the other end of the (fake) WebSocket.
struct Rig {
    hub: Hub,
    env: Fake,
    now: Instant,
    ws: WsId,
    /// Message ids of the `subscribe_trigger` requests, by entity.
    triggers: HashMap<String, u64>,
    /// What the fake answers to `call_service` (`None`: success).
    service_error: Option<&'static str>,
    trigger_error: Option<&'static str>,
    trigger_requests: Vec<Value>,
    unsubscribed: Vec<u64>,
    calls: Vec<Value>,
    /// What the fake answers to `get_states`.
    states: Vec<Value>,
    /// How many `get_states` requests were seen.
    get_states_requests: usize,
    services: Value,
    get_services_requests: usize,
    cache_id: Option<u64>,
    render_ids: Vec<u64>,
}

impl Rig {
    fn new() -> Self {
        let mut rig = Rig { hub: Hub::default(), env: Fake::default(), now: Instant::now(), ws: 0, triggers: HashMap::new(), service_error: None, trigger_error: None, trigger_requests: Vec::new(), unsubscribed: Vec::new(), calls: Vec::new(), states: Vec::new(), get_states_requests: 0, services: json!({}), get_services_requests: 0, cache_id: None, render_ids: Vec::new() };
        rig.hub.configure(URL, "tok", rig.now, &mut rig.env);
        rig
    }

    fn tick(&mut self, events: Vec<WsEvent>) -> Vec<Emission> {
        self.hub.tick(self.now, events, &mut self.env)
    }

    fn recv(&mut self, messages: Vec<Value>) -> Vec<Emission> {
        let ws = self.ws;
        self.tick(messages.into_iter().map(|m| WsEvent::Message(ws, m.to_string())).collect())
    }

    /// Opens the socket and authenticates, then answers everything the session asks for.
    fn connect(&mut self) -> Vec<Emission> {
        self.tick(vec![]);
        self.ws = self.env.next_ws;
        let ws = self.ws;
        self.tick(vec![WsEvent::Opened(ws)]);
        self.recv(vec![json!({"type": "auth_required", "ha_version": "2026.10"})]);
        assert_eq!(self.env.sent.remove(0), json!({"type": "auth", "access_token": "tok"}));
        self.recv(vec![json!({"type": "auth_ok", "ha_version": "2026.10"})]);
        self.serve()
    }

    /// Answers the requests the session has written until it is quiet; returns the emissions seen meanwhile.
    fn serve(&mut self) -> Vec<Emission> {
        let mut emissions = Vec::new();
        for _ in 0..10 {
            let sent = std::mem::take(&mut self.env.sent);
            if sent.is_empty() {
                break;
            }
            let mut replies = Vec::new();
            for request in sent {
                let id = request["id"].as_u64().unwrap();
                let ok = |result: Value| json!({"id": id, "type": "result", "success": true, "result": result});
                match request["type"].as_str().unwrap() {
                    "subscribe_trigger" => {
                        let trigger = &request["trigger"];
                        let entity = trigger["entity_id"].as_str().or_else(|| trigger["entity_id"][0].as_str()).unwrap_or_else(|| trigger["trigger"].as_str().unwrap());
                        self.triggers.insert(entity.to_string(), id);
                        self.trigger_requests.push(request.clone());
                        replies.push(match self.trigger_error {
                            None => ok(Value::Null),
                            Some(message) => json!({"id":id,"type":"result","success":false,"error":{"code":"invalid_format","message":message}}),
                        });
                    }
                    "subscribe_events" if request["event_type"] == "state_changed" => {
                        self.cache_id = Some(id);
                        replies.push(ok(Value::Null));
                    }
                    "get_config" => replies.push(ok(json!({"time_zone":"Europe/Kyiv"}))),
                    "render_template" => {
                        replies.push(ok(Value::Null));
                        if request["template"].as_str().unwrap().contains("utcoffset") {
                            replies.push(json!({"id":id,"type":"event","event":{"result":7200,"listeners":{}}}));
                        } else { self.render_ids.push(id); }
                    }
                    "unsubscribe_events" => {
                        self.unsubscribed.push(request["subscription"].as_u64().unwrap());
                        replies.push(ok(Value::Null));
                    }
                    "subscribe_entities" => {
                        let entity = request["entity_ids"][0].as_str().unwrap();
                        replies.push(ok(Value::Null));
                        replies.push(json!({"id": id, "type": "event", "event": {"a": {entity: {"s": "off", "a": {}, "c": "01X", "lc": 1.0, "lu": 1.0}}}}));
                    }
                    "call_service" => {
                        self.calls.push(request.clone());
                        replies.push(match self.service_error {
                            None => ok(if request["return_response"] == true { json!({"context":{},"response":{"answer":42}}) } else { Value::Null }),
                            Some(message) => json!({"id": id, "type": "result", "success": false, "error": {"code": "home_assistant_error", "message": message}}),
                        });
                    }
                    "ping" => replies.push(json!({"id": id, "type": "pong"})),
                    "get_services" => {
                        self.get_services_requests += 1;
                        replies.push(ok(self.services.clone()));
                    }
                    "get_states" => {
                        self.get_states_requests += 1;
                        replies.push(ok(Value::Array(self.states.clone())));
                    }
                    _ => replies.push(ok(Value::Null)),
                }
            }
            emissions.extend(self.recv(replies));
        }
        emissions
    }

    /// Lets `total` pass in 20 s steps, keeping the connection alive (the fake answers pings).
    fn advance(&mut self, total: Duration) {
        let mut left = total;
        while !left.is_zero() {
            let step = left.min(Duration::from_secs(20));
            self.now += step;
            left -= step;
            self.tick(vec![]);
            self.serve();
        }
    }

    /// A state change of `entity` as Home Assistant's trigger subscription reports it.
    fn change(&self, entity: &str, from: &str, to: &str) -> Value {
        json!({"id": self.triggers[entity], "type": "event", "event": {"variables": {"trigger": {
            "id": "0", "idx": "0", "alias": null, "platform": "state", "entity_id": entity,
            "from_state": ha_state(entity, from), "to_state": ha_state(entity, to), "for": null, "attribute": null,
        }}, "context": {"id": "01X", "parent_id": null, "user_id": null}}})
    }

    fn watch(&mut self, id: u128, entity: &str, mode: Mode) -> (NodeId, u64) {
        let node = node(id);
        let serial = self.hub.add_watch(node, mode);
        self.hub.set_entity(node, serial, entity, self.now, &mut self.env);
        (node, serial)
    }
}

fn data<'a>(outputs: &'a [Output], pin: PinId) -> &'a DataValue {
    outputs.iter().find_map(|o| match o {
        Output::Data(p, v) if *p == pin => Some(v),
        _ => None,
    }).unwrap_or_else(|| panic!("no data on pin {pin:?}: {outputs:?}"))
}

fn has_exec(outputs: &[Output], pin: PinId) -> bool {
    outputs.contains(&Output::Exec(pin))
}

fn state_of(value: &DataValue) -> String {
    ticknode_plugin_sdk::field_string(value, "state").unwrap().to_string()
}

#[test]
fn state_change_reaches_the_node() {
    let mut rig = Rig::new();
    let (a, _) = rig.watch(1, "light.a", Mode::Every);
    assert!(rig.connect().is_empty(), "the baseline read and the restored status are not changes");
    assert_eq!(rig.hub.connection_state(), Some(ConnectionState::Connected));
    assert_eq!(rig.env.connects, [URL]);

    let change = rig.change("light.a", "off", "on");
    let emissions = rig.recv(vec![change]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].node, a);
    let outputs = &emissions[0].outputs;
    assert!(has_exec(outputs, CHANGED));
    assert_eq!(state_of(data(outputs, NEW)), "on");
    assert_eq!(state_of(data(outputs, OLD)), "off");
    assert_eq!(data(outputs, NEW).get_type(), ticknode_plugin_sdk::DataType::Named("ha.state".into()));
    // Nothing more is delivered
    assert!(rig.recv(vec![]).is_empty());
}

#[test]
fn every_queues_across_ticks_and_latest_collapses() {
    let mut rig = Rig::new();
    let (every, _) = rig.watch(1, "light.a", Mode::Every);
    let (latest, _) = rig.watch(2, "light.b", Mode::Latest);
    rig.connect();
    let changes = vec![
        rig.change("light.a", "off", "on"), rig.change("light.a", "on", "off"), rig.change("light.a", "off", "on"),
        rig.change("light.b", "off", "on"), rig.change("light.b", "on", "off"), rig.change("light.b", "off", "on"),
    ];
    let first = rig.recv(changes);
    // One emission per node per tick
    assert_eq!(first.len(), 2);
    let of = |emissions: &[Emission], node: NodeId| emissions.iter().find(|e| e.node == node).map(|e| state_of(data(&e.outputs, NEW)));
    // Entity subscriptions merge changes that arrive together while a consumer lags, but all are queued
    // for `every` within one drain: the states arrive in order.
    let second = rig.recv(vec![]);
    let third = rig.recv(vec![]);
    let fourth = rig.recv(vec![]);
    let every_states: Vec<_> = [&first, &second, &third, &fourth].iter().filter_map(|e| of(e, every)).collect();
    assert_eq!(every_states, ["on", "off", "on"]);
    let latest_states: Vec<_> = [&first, &second, &third, &fourth].iter().filter_map(|e| of(e, latest)).collect();
    assert_eq!(latest_states, ["on"]);
    assert_eq!(of(&first, latest), Some("on".into()));
}

#[test]
fn absent_states_clear_change_and_trigger_outputs() {
    let change = StateChange { entity_id: "light.a".into(), old_state: None, new_state: None };
    let out = change_outputs(&change);
    assert!(out.contains(&Output::Clear(OLD)) && out.contains(&Output::Clear(NEW)));
    for platform in [crate::triggers::Platform::State, crate::triggers::Platform::NumericState, crate::triggers::Platform::Zone] {
        let properties = match platform {
            crate::triggers::Platform::NumericState => vec![("entity_id", "sensor.a"), ("above", "1")],
            crate::triggers::Platform::Zone => vec![("entity_id", "person.a"), ("zone", "zone.home")],
            _ => vec![("entity_id", "light.a")],
        };
        let config = trigger_config(platform, &properties);
        let fired = TriggerFired { trigger: json!({"entity_id":"light.a", "from_state":null, "to_state":null}), context: None };
        let out = trigger_outputs(&config, &fired);
        assert!(out.contains(&Output::Clear(PinId(3))) && out.contains(&Output::Clear(PinId(4))));
        if platform == crate::triggers::Platform::NumericState {
            assert!(out.contains(&Output::Clear(PinId(5))));
        }
    }
}

#[test]
fn requests_fail_promptly_when_not_connected() {
    let mut rig = Rig::new();
    // Configured but the socket is not up yet
    let outputs = rig.hub.get_state(node(9), "light.a", rig.now, &mut rig.env).expect("an immediate answer");
    assert!(has_exec(&outputs, GET_FAILED));
    assert!(outputs.contains(&Output::Clear(GET_STATE)));
    let outputs = rig.hub.call_service(node(9), "light", "turn_on", "", Value::Null, rig.now, &mut rig.env).expect("an immediate answer");
    assert!(has_exec(&outputs, CALL_FAILED));
    assert!(matches!(data(&outputs, CALL_ERROR), DataValue::String(e) if e.contains("not connected")));
    // Not configured at all
    let mut hub = Hub::default();
    let outputs = hub.get_state(node(9), "light.a", rig.now, &mut rig.env).unwrap();
    assert!(matches!(data(&outputs, GET_ERROR), DataValue::String(e) if e.contains("not configured")));
}

#[test]
fn get_state_is_answered_in_a_later_tick() {
    let mut rig = Rig::new();
    rig.connect();
    assert!(rig.hub.get_state(node(5), "light.a", rig.now, &mut rig.env).is_none());
    let emissions = rig.serve();
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].node, node(5));
    assert!(has_exec(&emissions[0].outputs, GET_DONE));
    assert_eq!(state_of(data(&emissions[0].outputs, GET_STATE)), "off");
    assert_eq!(data(&emissions[0].outputs, GET_ERROR), &DataValue::String(String::new()));
}

#[test]
fn call_service_sends_the_payload_and_reports_the_result() {
    let mut rig = Rig::new();
    rig.connect();
    assert!(rig.hub.call_service(node(5), "light", "turn_on", "light.a", json!({"brightness": 120}), rig.now, &mut rig.env).is_none());
    let emissions = rig.serve();
    assert_eq!(rig.calls, [json!({
        "id": rig.calls[0]["id"], "type": "call_service", "domain": "light", "service": "turn_on",
        "service_data": {"brightness": 120}, "target": {"entity_id": ["light.a"]},
    })]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, CALL_DONE));

    rig.service_error = Some("nope");
    assert!(rig.hub.call_service(node(5), "light", "turn_on", "", Value::Null, rig.now, &mut rig.env).is_none());
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs, CALL_FAILED));
    assert!(matches!(data(&emissions[0].outputs, CALL_ERROR), DataValue::String(e) if e.contains("nope")));
    // No target and no data: only the required fields are sent
    assert_eq!(rig.calls[1], json!({"id": rig.calls[1]["id"], "type": "call_service", "domain": "light", "service": "turn_on"}));
    // A non-object payload is rejected before anything is sent
    let outputs = rig.hub.call_service(node(5), "light", "turn_on", "", json!([1]), rig.now, &mut rig.env).unwrap();
    assert!(has_exec(&outputs, CALL_FAILED));
}

#[test]
fn reconnects_with_backoff_and_ignores_stale_sockets() {
    let mut rig = Rig::new();
    rig.watch(1, "light.a", Mode::Every);
    rig.connect();
    let old = rig.ws;
    rig.tick(vec![WsEvent::Closed(old, "gone".into())]);
    assert!(matches!(rig.hub.connection_state(), Some(ConnectionState::Reconnecting { .. })));
    assert!(rig.env.logs.iter().any(|l| l.starts_with("warn: Not connected to Home Assistant")));
    // Requests fail at once while disconnected
    assert!(has_exec(&rig.hub.get_state(node(5), "light.a", rig.now, &mut rig.env).unwrap(), GET_FAILED));
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
    assert_eq!(options.iter().map(|o| (o.value.as_str(), o.label.as_str(), o.icon.as_deref())).collect::<Vec<_>>(), [
        ("light.kitchen", "Lamp", Some("mdi:lightbulb")),
        ("binary_sensor.door", "Door", Some("mdi:door-open")),
    ]);
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
    let id = rig.env.sent.iter().find(|m| m["type"] == "get_states").unwrap()["id"].as_u64().unwrap();
    rig.env.sent.clear();
    rig.tick(vec![WsEvent::Message(ws, json!({"id": id, "type": "result", "success": false, "error": {"code": "boom", "message": "no"}}).to_string())]);
    assert!(rig.env.logs.iter().any(|l| l.starts_with("warn: Could not read the list of entities")), "{:?}", rig.env.logs);
    assert_eq!(rig.env.option_calls, calls);
    rig.advance(ENTITIES_RETRY);
    assert_eq!(rig.env.option_calls, calls + 1);
}

#[test]
fn failed_connect_is_retried_and_a_bad_token_stops() {
    let mut rig = Rig::new();
    rig.env.fail_connect = true;
    rig.tick(vec![]);
    assert!(rig.env.logs.iter().any(|l| l.contains("Could not open the Home Assistant WebSocket: refused")));
    rig.env.fail_connect = false;
    rig.now += Duration::from_secs(3);
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 1);
    rig.ws = rig.env.next_ws;
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Opened(ws)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    rig.recv(vec![json!({"type": "auth_invalid", "message": "Invalid access token"})]);
    rig.tick(vec![]);
    assert!(matches!(rig.hub.connection_state(), Some(ConnectionState::AuthFailed(_))));
    assert!(rig.env.logs.iter().any(|l| l.starts_with("error: Home Assistant rejected the access token")));
    assert!(rig.env.closed.contains(&ws), "the socket is dropped");
    rig.now += Duration::from_secs(3600);
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.len(), 1, "never retried");
}

#[test]
fn reconfiguring_closes_the_old_socket_and_resubscribes() {
    let mut rig = Rig::new();
    rig.watch(1, "light.a", Mode::Every);
    rig.connect();
    let old = rig.ws;
    rig.hub.configure("ws://other/api/websocket", "tok2", rig.now, &mut rig.env);
    assert!(rig.env.closed.contains(&old));
    rig.tick(vec![]);
    assert_eq!(rig.env.connects.last().unwrap(), "ws://other/api/websocket");
    // The old socket's events no longer matter
    rig.tick(vec![WsEvent::Message(old, json!({"type": "auth_required"}).to_string())]);
    assert!(rig.env.sent.is_empty());
    rig.ws = rig.env.next_ws;
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Opened(ws)]);
    rig.recv(vec![json!({"type": "auth_required"})]);
    assert_eq!(rig.env.sent.remove(0), json!({"type": "auth", "access_token": "tok2"}));
    rig.recv(vec![json!({"type": "auth_ok"})]);
    rig.serve();
    let change = rig.change("light.a", "off", "on");
    assert_eq!(rig.recv(vec![change]).len(), 1);
}

#[test]
fn changing_the_entity_and_dropping_the_node_unsubscribe() {
    let mut rig = Rig::new();
    let (a, serial) = rig.watch(1, "light.a", Mode::Every);
    rig.connect();
    let first_trigger = rig.triggers["light.a"];

    rig.hub.set_entity(a, serial, "light.b", rig.now, &mut rig.env);
    // The old subscription is cancelled and a new one is made
    let sent = rig.env.sent.clone();
    assert!(sent.iter().any(|m| m["type"] == "unsubscribe_events" && m["subscription"] == first_trigger), "{sent:?}");
    assert!(sent.iter().any(|m| m["type"] == "subscribe_trigger" && m["trigger"]["entity_id"] == "light.b"), "{sent:?}");
    rig.serve();
    let stale = rig.change("light.a", "off", "on");
    assert!(rig.recv(vec![stale]).is_empty(), "the old entity no longer reaches the node");
    let change = rig.change("light.b", "off", "on");
    assert_eq!(rig.recv(vec![change]).len(), 1);

    // A stale serial cannot remove the node
    // (Home Assistant's late event for the cancelled subscription makes the session unsubscribe once more)
    rig.env.sent.clear();
    rig.hub.remove_watch(a, serial + 100, rig.now, &mut rig.env);
    assert!(rig.env.sent.is_empty());
    let second_trigger = rig.triggers["light.b"];
    rig.hub.remove_watch(a, serial, rig.now, &mut rig.env);
    assert!(rig.env.sent.iter().any(|m| m["type"] == "unsubscribe_events" && m["subscription"] == second_trigger));
    rig.serve();
    let late = rig.change("light.b", "on", "off");
    assert!(rig.recv(vec![late]).is_empty());
}

#[test]
fn a_failed_subscription_is_logged_and_retried() {
    let mut rig = Rig::new();
    rig.watch(1, "light.ghost", Mode::Every);
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
    assert!(rig.env.logs.iter().any(|l| l.starts_with("error: Home Assistant rejected the subscription to light.ghost")), "{:?}", rig.env.logs);
    // After the delay the node subscribes again
    assert!(rig.env.sent.is_empty());
    rig.now += RESUBSCRIBE_DELAY + Duration::from_secs(1);
    rig.tick(vec![]);
    rig.tick(vec![]);
    assert!(rig.env.sent.iter().any(|m| m["type"] == "subscribe_trigger"), "{:?}", rig.env.sent);
}

fn trigger_config(platform: crate::triggers::Platform, values: &[(&str, &str)]) -> TriggerConfig {
    let mut props = platform.defaults();
    props.extend(values.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    crate::triggers::build(platform, &props).unwrap().unwrap()
}

fn fire(id: u64, trigger: Value) -> Value {
    json!({"id":id,"type":"event","event":{"variables":{"trigger":trigger}}})
}

#[test]
fn trigger_replaces_subscription_clears_queue_and_unsubscribes_on_removal() {
    use crate::triggers::{INFO, Platform, TRIGGERED};
    let mut rig = Rig::new();
    rig.connect();
    let a = node(500);
    let serial = rig.hub.add_trigger(a, rig.now);
    let config = trigger_config(Platform::State, &[("entity_id", "light.a"), ("to", "on")]);
    rig.hub.set_trigger(a, serial, Some(config.clone()), rig.now, &mut rig.env);
    rig.serve();
    let first = rig.triggers["light.a"];
    assert_eq!(rig.trigger_requests.last().unwrap()["trigger"], json!({"trigger":"state","entity_id":["light.a"],"to":"on"}));
    let fired = |id, n| fire(id, json!({"entity_id":"light.a","platform":"state","description":n}));
    let emissions = rig.recv(vec![fired(first, 1), fired(first, 2), fired(first, 3)]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(emissions[0].node, a);
    assert!(has_exec(&emissions[0].outputs, TRIGGERED));
    assert_eq!(ticknode_plugin_sdk::field(data(&emissions[0].outputs, INFO), "description"), Some(&DataValue::from(1i64)));
    let emissions = rig.tick(vec![]);
    assert_eq!(emissions.len(), 1);
    assert_eq!(ticknode_plugin_sdk::field(data(&emissions[0].outputs, INFO), "description"), Some(&DataValue::from(2i64)));
    let sent = rig.trigger_requests.len();
    rig.hub.set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.serve();
    assert_eq!(rig.trigger_requests.len(), sent, "equivalent config keeps the subscription");
    // Queue another fire, then invalidate the config. No queued old fire may escape.
    rig.recv(vec![fired(first, 4), fired(first, 5)]);
    rig.hub.set_trigger(a, serial, None, rig.now, &mut rig.env);
    assert!(rig.serve().is_empty());
    assert!(rig.unsubscribed.contains(&first));
    assert!(rig.tick(vec![]).is_empty());
    let config = trigger_config(Platform::State, &[("entity_id", "light.b")]);
    rig.hub.set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.serve();
    let second = rig.triggers["light.b"];
    assert_ne!(first, second);
    assert!(rig.recv(vec![fired(first, 6)]).is_empty());
    rig.hub.remove_trigger(a, serial + 1, rig.now, &mut rig.env);
    assert!(rig.hub.triggers.contains_key(&a), "late drop cannot remove another generation");
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
    rig.hub.set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
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
    assert_eq!(rig.recv(vec![fire(rig.triggers["template"], json!({"platform":"template"}))]).len(), 1);
}

#[test]
fn offline_trigger_rejection_retries_and_reconfiguration_restores_it() {
    use crate::triggers::Platform;
    let mut rig = Rig::new();
    let a = node(503);
    let serial = rig.hub.add_trigger(a, rig.now);
    let config = trigger_config(Platform::Event, &[("event_type", "button")]);
    rig.hub.set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    rig.trigger_error = Some("not allowed");
    rig.connect();
    let errors = rig.hub.take_trigger_errors(a, serial);
    assert!(errors.iter().any(|e| e.contains("rejected") && e.contains("not allowed")), "{errors:?}");
    rig.trigger_error = None;
    rig.advance(RESUBSCRIBE_DELAY);
    assert_eq!(rig.trigger_requests.len(), 2);
    // Reconfigure the connection: the same node subscribes in the new session.
    rig.hub.configure(URL, "tok", rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(rig.trigger_requests.len(), 3);
    assert_eq!(rig.recv(vec![fire(rig.triggers["event"], json!({"platform":"event"}))]).len(), 1);
}

#[test]
fn trigger_edits_and_removal_before_the_first_tick_cancel_ready_pending_subscriptions() {
    use crate::triggers::Platform;
    let mut rig = Rig::new();
    let a = node(504);
    let serial = rig.hub.add_trigger(a, rig.now);
    for to in ["off", "on", "unknown"] {
        let config = trigger_config(Platform::State, &[("entity_id", "light.a"), ("to", to)]);
        rig.hub.set_trigger(a, serial, Some(config), rig.now, &mut rig.env);
    }
    let b = node(505);
    let removed = rig.hub.add_trigger(b, rig.now);
    let config = trigger_config(Platform::Event, &[("event_type", "removed")]);
    rig.hub.set_trigger(b, removed, Some(config), rig.now, &mut rig.env);
    rig.hub.remove_trigger(b, removed, rig.now, &mut rig.env);
    rig.connect();
    assert_eq!(rig.trigger_requests.len(), 1, "only the final config is sent after connecting");
    assert_eq!(rig.trigger_requests[0]["trigger"], json!({"trigger":"state","entity_id":["light.a"],"to":"unknown"}));
}

#[test]
fn actions_refresh_and_calls_request_response_only_when_supported() {
    use crate::actions::{DONE, FAILED, RESPONSE};
    let mut rig = Rig::new();
    rig.services = json!({"light":{"turn_on":{"name":"Turn on","target":{}}},"weather":{"get_forecasts":{"response":{"optional":false}}}});
    rig.states = vec![ha_state("light.a", "off"), ha_state("switch.a", "off")];
    rig.connect();
    assert_eq!(rig.get_services_requests,1);
    assert_eq!(rig.env.options["actions"].iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),["weather.get_forecasts"]);
    assert_eq!(rig.env.options["action_targets"].iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),["light.a"]);
    assert_eq!(rig.env.options["actions:light.a"].iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),["light.turn_on"]);
    assert!(rig.hub.action(node(5), "light", "turn_on", "light.a", json!({"brightness":120}), false, rig.now, &mut rig.env).is_none());
    let emissions = rig.serve();
    assert!(has_exec(&emissions[0].outputs,DONE));
    assert!(rig.calls[0].get("return_response").is_none());
    assert!(rig.hub.action(node(5), "weather", "get_forecasts", "weather.home", json!({}), false, rig.now, &mut rig.env).is_none());
    let emissions = rig.serve();
    assert_eq!(rig.calls[1]["return_response"],true);
    assert_eq!(ticknode_plugin_sdk::field_number(data(&emissions[0].outputs,RESPONSE),"answer"),Some(42.0));
    rig.advance(ENTITIES_REFRESH);
    assert_eq!(rig.get_services_requests,2);
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Closed(ws,"gone".into())]);
    let out = rig.hub.action(node(5), "light", "turn_on", "", json!({}), false, rig.now, &mut rig.env).expect("fails promptly");
    assert!(has_exec(&out,FAILED));
    rig.now += Duration::from_secs(3);
    rig.connect();
    assert_eq!(rig.get_services_requests,3);
}

#[test]
fn entity_data_has_an_initial_baseline_changes_and_a_reconnected_baseline() {
    let mut rig = Rig::new();
    let id = node(20);
    let serial = rig.hub.add_entity(id);
    rig.hub.set_entity(id,serial,"light.a",rig.now,&mut rig.env);
    let emissions = rig.connect();
    assert_eq!(emissions.len(),1);
    assert_eq!(data(&emissions[0].outputs,PinId(0)), &DataValue::from("off"));
    assert_eq!(data(&emissions[0].outputs,PinId(2)), &DataValue::Bool(false));
    assert!(emissions[0].outputs.iter().all(|o| matches!(o, Output::Data(..) | Output::Clear(..))));
    assert!(emissions[0].outputs.contains(&Output::Clear(PinId(1))));
    let change = rig.change("light.a","off","on");
    let emissions = rig.recv(vec![change]);
    assert_eq!(data(&emissions[0].outputs,PinId(2)), &DataValue::Bool(true));
    let ws = rig.ws;
    rig.tick(vec![WsEvent::Closed(ws,"gone".into())]);
    // The scripted fake's current baseline is off. Reconnect must publish it again.
    rig.now += Duration::from_secs(3);
    let emissions = rig.connect();
    assert!(emissions.iter().any(|e| e.node == id && e.outputs.iter().any(|o| matches!(o,Output::Data(p,v) if p.0 == 0 && *v == DataValue::from("off")))));
    rig.hub.remove_watch(id,serial,rig.now,&mut rig.env);
    assert!(!rig.hub.entities.contains_key(&id));
}

#[test]
fn live_cache_applies_pushes_removals_and_invalidates_on_disconnect() {
    let mut rig = Rig::new();
    rig.states = vec![ha_state("light.test", "off")];
    rig.connect();
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "off");
    assert_eq!(rig.hub.live.offset().unwrap().local_minus_utc(), 7200);
    let id = rig.cache_id.unwrap();
    let event = |state: Value| json!({"id":id,"type":"event","event":{"event_type":"state_changed",
        "data":{"entity_id":"light.test","old_state":ha_state("light.test","off"),"new_state":state},
        "origin":"LOCAL","time_fired":"2026-10-30T10:00:00Z","context":{"id":"t"}}});
    rig.recv(vec![event(ha_state("light.test","on"))]);
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "on");
    // An event racing a get_states response wins over the older snapshot.
    rig.hub.live.begin_snapshot();
    rig.recv(vec![event(ha_state("light.test","newer"))]);
    rig.hub.live.snapshot(&[serde_json::from_value(ha_state("light.test","old")).unwrap()]);
    assert_eq!(rig.hub.live.state("light.test").unwrap().state, "newer");
    rig.hub.live.begin_snapshot();
    rig.recv(vec![event(Value::Null)]);
    rig.hub.live.snapshot(&[serde_json::from_value(ha_state("light.test","old")).unwrap()]);
    assert!(rig.hub.live.state("light.test").is_err(), "removal must win over a stale snapshot");
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
        assert!(rig.hub.render_condition(node, serial, "{{ test }}", rig.now, &mut rig.env).is_empty());
        rig.serve();
        *rig.render_ids.last().unwrap()
    };
    let id = start(&mut rig);
    let event = |id, result| json!({"id":id,"type":"event","event":{"result":result,"listeners":{}}});
    let emissions = rig.recv(vec![event(id, json!(true)),event(id, json!(false))]);
    assert_eq!(emissions.len(), 1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::YES));
    assert_eq!(data(&emissions[0].outputs, crate::conditions::RESULT), &DataValue::Bool(true));
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    let id = start(&mut rig);
    let emissions = rig.recv(vec![json!({"id":id,"type":"event","event":{"error":"render failed","level":"ERROR"}})]);
    assert_eq!(emissions.len(),1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
    let id = start(&mut rig);
    rig.now += Duration::from_secs(31);
    let emissions = rig.tick(vec![]);
    assert_eq!(emissions.len(),1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    let id = start(&mut rig);
    rig.hub.cancel_templates(node,serial,rig.now,&mut rig.env);
    assert!(rig.hub.templates.is_empty());
    assert!(rig.recv(vec![event(id,json!(true))]).is_empty());
    rig.serve();
    assert!(rig.unsubscribed.contains(&id));
    start(&mut rig);
    let emissions = rig.tick(vec![WsEvent::Closed(rig.ws,"lost".into())]);
    assert_eq!(emissions.len(),1);
    assert!(has_exec(&emissions[0].outputs, crate::conditions::NO));
}

#[test]
fn state_and_attribute_options_publish_for_tracked_nodes() {
    let mut rig = Rig::new();
    rig.states = vec![
        ha_state("light.hall", "on"),
        ha_state("cover.garage", "open"),
    ];
    rig.connect();

    assert!(rig.env.options.contains_key("states"));
    assert!(rig.env.options.contains_key("attributes"));

    let node1 = node(10);
    let node2 = node(20);

    rig.hub.set_state_node_entity(node1, "light.hall", &mut rig.env);
    assert!(rig.env.options.contains_key("states:light.hall"));
    assert!(rig.env.options.contains_key("attributes:light.hall"));
    assert!(rig.env.options["states:light.hall"].iter().any(|o| o.value == "on" && o.label.contains("Поточний стан")));

    rig.hub.set_state_node_entity(node2, "light.hall", &mut rig.env);

    rig.hub.remove_state_node(node1, &mut rig.env);
    assert!(!rig.env.options["states:light.hall"].is_empty());

    rig.hub.remove_state_node(node2, &mut rig.env);
    assert!(rig.env.options["states:light.hall"].is_empty());
    assert!(rig.env.options["attributes:light.hall"].is_empty());
}
