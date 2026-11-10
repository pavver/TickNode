use super::*;
use crate::entities::ENTITIES_SOURCE;
use serde_json::{Value, json};
use ticknode_plugin_sdk::{DataValue, Emission, Output, PinId, WsEvent};
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
        if source == ENTITIES_SOURCE {
            self.option_calls += 1;
        }
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
        let mut rig = Rig {
            hub: Hub::default(),
            env: Fake::default(),
            now: Instant::now(),
            ws: 0,
            triggers: HashMap::new(),
            service_error: None,
            trigger_error: None,
            trigger_requests: Vec::new(),
            unsubscribed: Vec::new(),
            calls: Vec::new(),
            states: Vec::new(),
            get_states_requests: 0,
            services: json!({}),
            get_services_requests: 0,
            cache_id: None,
            render_ids: Vec::new(),
        };
        rig.hub.configure(URL, "tok", rig.now, &mut rig.env);
        rig
    }

    fn tick(&mut self, events: Vec<WsEvent>) -> Vec<Emission> {
        self.hub.tick(self.now, events, &mut self.env)
    }

    fn recv(&mut self, messages: Vec<Value>) -> Vec<Emission> {
        let ws = self.ws;
        self.tick(
            messages
                .into_iter()
                .map(|m| WsEvent::Message(ws, m.to_string()))
                .collect(),
        )
    }

    /// Opens the socket and authenticates, then answers everything the session asks for.
    fn connect(&mut self) -> Vec<Emission> {
        self.tick(vec![]);
        self.ws = self.env.next_ws;
        let ws = self.ws;
        self.tick(vec![WsEvent::Opened(ws)]);
        self.recv(vec![
            json!({"type": "auth_required", "ha_version": "2026.10"}),
        ]);
        assert_eq!(
            self.env.sent.remove(0),
            json!({"type": "auth", "access_token": "tok"})
        );
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
                        let entity = trigger["entity_id"]
                            .as_str()
                            .or_else(|| trigger["entity_id"][0].as_str())
                            .unwrap_or_else(|| {
                                trigger["event_type"][0]
                                    .as_str()
                                    .or_else(|| trigger["trigger"].as_str())
                                    .unwrap()
                            });
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
                        } else {
                            self.render_ids.push(id);
                        }
                    }
                    "unsubscribe_events" => {
                        self.unsubscribed
                            .push(request["subscription"].as_u64().unwrap());
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

    fn watch(&mut self, id: u128, entity: &str) -> (NodeId, u64) {
        let node = node(id);
        let serial = self.hub.add_entity(node);
        self.hub
            .set_entity(node, serial, entity, self.now, &mut self.env);
        (node, serial)
    }
}

fn data<'a>(outputs: &'a [Output], pin: PinId) -> &'a DataValue {
    outputs
        .iter()
        .find_map(|o| match o {
            Output::Data(p, v) if *p == pin => Some(v),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no data on pin {pin:?}: {outputs:?}"))
}

fn has_exec(outputs: &[Output], pin: PinId) -> bool {
    outputs.contains(&Output::Exec(pin))
}

fn trigger_config(platform: crate::triggers::Platform, values: &[(&str, &str)]) -> TriggerConfig {
    let mut props = platform.defaults();
    props.extend(values.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    crate::triggers::build(platform, &props).unwrap().unwrap()
}

fn fire(id: u64, trigger: Value) -> Value {
    json!({"id":id,"type":"event","event":{"variables":{"trigger":trigger}}})
}

mod catalog;
mod conditions;
mod entity;
mod transport;
mod triggers;
