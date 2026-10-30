//! Plugin-level state: the single Home Assistant connection (a sans-IO `Session`), the per-node
//! subscriptions and the in-flight requests. It talks to the outside world only through [`Env`], and the
//! clock is passed in, so the whole logic runs in tests on the host target.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ha_core_client::{
    Action, ClientConfig, ConnectionState, PendingSubscription, Reply, Session, Source, State, StateChange, Target,
    Update, SubscriptionStatus,
};
use ticknode_plugin_sdk::{Emission, NodeId, Output, PinId, WsEvent, WsId};

use crate::convert::{missing_state, state_value};
use crate::queue::{ChangeQueue, Mode};

/// Output pins of `ha/state_changed`.
pub const CHANGED: PinId = PinId(0);
pub const NEW: PinId = PinId(1);
pub const OLD: PinId = PinId(2);
/// Output pins of `ha/get_state`.
pub const GET_DONE: PinId = PinId(2);
pub const GET_FAILED: PinId = PinId(3);
pub const GET_STATE: PinId = PinId(4);
pub const GET_ERROR: PinId = PinId(5);
/// Output pins of `ha/call_service`.
pub const CALL_DONE: PinId = PinId(5);
pub const CALL_FAILED: PinId = PinId(6);
pub const CALL_ERROR: PinId = PinId(7);

/// How long to wait before subscribing again after a subscription failed (for example, the entity does not exist).
const RESUBSCRIBE_DELAY: Duration = Duration::from_secs(30);

/// What the hub needs from the host.
pub trait Env {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String>;
    fn ws_send(&mut self, id: WsId, text: &str) -> Result<(), String>;
    fn ws_close(&mut self, id: WsId);
    fn info(&mut self, message: &str);
    fn warn(&mut self, message: &str);
    fn error(&mut self, message: &str);
}

struct Link {
    url: String,
    session: Session,
    ws: Option<WsId>,
}

enum Sub {
    /// Not subscribed (no entity yet, or a new one was just set).
    Idle,
    Pending(PendingSubscription<StateChange>),
    Active(ha_core_client::SessionEvents<StateChange>),
    /// The subscription failed or ended; try again at `retry_at`.
    Failed { retry_at: Instant },
}

struct Watch {
    /// Distinguishes a node from an older one with the same id, so a late `Drop` cannot remove the new one.
    serial: u64,
    entity: String,
    queue: ChangeQueue<StateChange>,
    sub: Sub,
}

enum Call {
    Get { entity: String, reply: Reply<State> },
    Service { reply: Reply<()> },
}

struct InFlight {
    node: NodeId,
    call: Call,
}

#[derive(Default)]
pub struct Hub {
    link: Option<Link>,
    watches: HashMap<NodeId, Watch>,
    requests: Vec<InFlight>,
    next_serial: u64,
    /// The last connection state that was logged.
    logged: Option<ConnectionState>,
}

/// `Output`s of a `ha/state_changed` pulse. A missing state is sent as an empty `ha.state` (see [`missing_state`]).
pub fn change_outputs(change: &StateChange) -> Vec<Output> {
    let value = |state: &Option<State>| state.as_ref().map_or_else(|| missing_state(&change.entity_id), state_value);
    vec![Output::Data(NEW, value(&change.new_state)), Output::Data(OLD, value(&change.old_state)), Output::Exec(CHANGED)]
}

pub fn get_state_outputs(entity: &str, result: Result<State, String>) -> Vec<Output> {
    match result {
        Ok(state) => vec![Output::Data(GET_STATE, state_value(&state)), Output::Data(GET_ERROR, "".into()), Output::Exec(GET_DONE)],
        Err(error) => vec![Output::Data(GET_STATE, missing_state(entity)), Output::Data(GET_ERROR, error.into()), Output::Exec(GET_FAILED)],
    }
}

pub fn service_outputs(result: Result<(), String>) -> Vec<Output> {
    match result {
        Ok(()) => vec![Output::Data(CALL_ERROR, "".into()), Output::Exec(CALL_DONE)],
        Err(error) => vec![Output::Data(CALL_ERROR, error.into()), Output::Exec(CALL_FAILED)],
    }
}

const NOT_CONFIGURED: &str = "Home Assistant is not configured (set the URL and the access token in the plugin settings)";

impl Hub {
    // --- Configuration ---

    /// Closes the connection and forgets the session. Subscriptions are dropped (they belong to the session);
    /// requests still in flight fail with "stopped" on the next tick.
    pub fn reset(&mut self, now: Instant, env: &mut impl Env) {
        if let Some(mut link) = self.link.take() {
            link.session.stop(now);
            if let Some(id) = link.ws.take() {
                env.ws_close(id);
            }
            // The stop must reach the in-flight requests' replies, which are polled on the next tick.
        }
        for watch in self.watches.values_mut() {
            watch.sub = Sub::Idle;
        }
        self.logged = None;
    }

    /// (Re)creates the session for new settings; the old socket is closed.
    pub fn configure(&mut self, url: &str, token: &str, now: Instant, env: &mut impl Env) {
        self.reset(now, env);
        self.link = Some(Link { url: url.to_string(), session: Session::new(ClientConfig::for_session(token)), ws: None });
        env.info(&format!("Home Assistant connection configured for {url}"));
        self.ensure_subscriptions(now);
    }

    // --- Nodes ---

    /// Registers a `ha/state_changed` node; returns its serial.
    pub fn add_watch(&mut self, node: NodeId, mode: Mode) -> u64 {
        self.next_serial += 1;
        let serial = self.next_serial;
        self.watches.insert(node, Watch { serial, entity: String::new(), queue: ChangeQueue::new(mode), sub: Sub::Idle });
        serial
    }

    /// Unregisters the node and cancels its subscription.
    pub fn remove_watch(&mut self, node: NodeId, serial: u64, now: Instant, env: &mut impl Env) {
        if self.watches.get(&node).is_some_and(|w| w.serial == serial) {
            if let Some(watch) = self.watches.remove(&node) {
                self.cancel(watch.sub, now);
            }
            self.flush(now, env);
        }
    }

    pub fn set_entity(&mut self, node: NodeId, serial: u64, entity: &str, now: Instant, env: &mut impl Env) {
        let Some(watch) = self.watches.get_mut(&node).filter(|w| w.serial == serial) else { return };
        if watch.entity == entity {
            return;
        }
        watch.entity = entity.to_string();
        watch.queue.clear();
        let old = std::mem::replace(&mut watch.sub, Sub::Idle);
        self.cancel(old, now);
        self.ensure_subscriptions(now);
        self.flush(now, env);
    }

    pub fn set_mode(&mut self, node: NodeId, serial: u64, mode: Mode) {
        if let Some(watch) = self.watches.get_mut(&node).filter(|w| w.serial == serial) {
            watch.queue.set_mode(mode);
        }
    }

    fn cancel(&mut self, sub: Sub, now: Instant) {
        if let (Sub::Active(events), Some(link)) = (&sub, &mut self.link) {
            // The reply is not needed: the session cleans up on its own.
            let _ = link.session.unsubscribe(now, events.subscription());
        }
        // A pending subscription is abandoned by dropping it.
    }

    // --- Requests ---

    /// Starts reading a state. `Some(outputs)` when the result is already known (for example a failure
    /// while disconnected), otherwise the node gets its outputs from a later emission.
    pub fn get_state(&mut self, node: NodeId, entity: &str, now: Instant, env: &mut impl Env) -> Option<Vec<Output>> {
        let Some(link) = &mut self.link else {
            return Some(get_state_outputs(entity, Err(NOT_CONFIGURED.into())));
        };
        let mut reply = link.session.get_state(now, entity);
        let first = reply.poll();
        let immediate = match first {
            Some(result) => Some(get_state_outputs(entity, result.map_err(|e| e.to_string()))),
            None => {
                self.requests.push(InFlight { node, call: Call::Get { entity: entity.to_string(), reply } });
                None
            }
        };
        self.flush(now, env);
        immediate
    }

    pub fn call_service(
        &mut self,
        node: NodeId,
        domain: &str,
        service: &str,
        entity: &str,
        data: serde_json::Value,
        now: Instant,
        env: &mut impl Env,
    ) -> Option<Vec<Output>> {
        let Some(link) = &mut self.link else {
            return Some(service_outputs(Err(NOT_CONFIGURED.into())));
        };
        let target = if entity.is_empty() { Target::new() } else { Target::entity(entity) };
        let mut reply = match link.session.call_service(now, domain, service, target, data) {
            Ok(reply) => reply,
            Err(e) => return Some(service_outputs(Err(e.to_string()))),
        };
        let immediate = match reply.poll() {
            Some(result) => Some(service_outputs(result.map_err(|e| e.to_string()))),
            None => {
                self.requests.push(InFlight { node, call: Call::Service { reply } });
                None
            }
        };
        self.flush(now, env);
        immediate
    }

    // --- Tick ---

    /// One plugin tick: feeds the connection events to the session, runs the transport, collects
    /// subscription updates and request results and turns them into emissions (at most one per node).
    pub fn tick(&mut self, now: Instant, events: Vec<WsEvent>, env: &mut impl Env) -> Vec<Emission> {
        if let Some(link) = &mut self.link {
            for event in events {
                match event {
                    WsEvent::Opened(id) if link.ws == Some(id) => link.session.on_connected(now),
                    WsEvent::Message(id, text) if link.ws == Some(id) => link.session.on_frame(now, &text),
                    WsEvent::Closed(id, reason) if link.ws == Some(id) => {
                        link.ws = None;
                        link.session.on_disconnected(now, &reason);
                    }
                    // Events of a connection that was replaced or closed by us
                    _ => {}
                }
            }
            Self::pump_actions(link, now, env);
            link.session.tick(now);
            Self::pump_actions(link, now, env);
        }
        self.ensure_subscriptions(now);
        let emissions = self.collect(now, env);
        self.flush(now, env);
        self.log_connection_state(env);
        emissions
    }

    fn pump_actions(link: &mut Link, now: Instant, env: &mut impl Env) {
        while let Some(action) = link.session.poll_action(now) {
            match action {
                Action::Connect => {
                    if let Some(old) = link.ws.take() {
                        env.ws_close(old);
                    }
                    match env.ws_connect(&link.url) {
                        Ok(id) => link.ws = Some(id),
                        Err(e) => {
                            env.warn(&format!("Could not open the Home Assistant WebSocket: {e}"));
                            link.session.on_disconnected(now, &e);
                        }
                    }
                }
                Action::Close => {
                    if let Some(id) = link.ws.take() {
                        env.ws_close(id);
                    }
                }
                _ => {}
            }
        }
    }

    /// Sends what the session wants to write.
    fn flush(&mut self, now: Instant, env: &mut impl Env) {
        let Some(link) = &mut self.link else { return };
        while let Some(frame) = link.session.poll_transmit() {
            let Some(id) = link.ws else { continue };
            if let Err(e) = env.ws_send(id, &frame) {
                env.warn(&format!("Writing to the Home Assistant WebSocket failed: {e}"));
                link.session.on_disconnected(now, &e);
                break;
            }
        }
    }

    fn ensure_subscriptions(&mut self, now: Instant) {
        let Some(link) = &mut self.link else { return };
        for watch in self.watches.values_mut() {
            let due = match watch.sub {
                Sub::Idle => true,
                Sub::Failed { retry_at } => now >= retry_at,
                Sub::Pending(_) | Sub::Active(_) => false,
            };
            if due && !watch.entity.is_empty() {
                watch.sub = Sub::Pending(link.session.subscribe(now, Source::entity(watch.entity.as_str()).without_merging()));
            }
        }
    }

    fn collect(&mut self, now: Instant, env: &mut impl Env) -> Vec<Emission> {
        let mut emissions = Vec::new();
        for (node, watch) in &mut self.watches {
            if let Sub::Pending(pending) = &mut watch.sub {
                match pending.poll() {
                    None => {}
                    Some(Ok(events)) => watch.sub = Sub::Active(events),
                    Some(Err(e)) => {
                        env.warn(&format!("Subscribing to {} failed: {e}", watch.entity));
                        watch.sub = Sub::Failed { retry_at: now + RESUBSCRIBE_DELAY };
                    }
                }
            }
            if let Sub::Active(events) = &mut watch.sub {
                for update in events.drain() {
                    match update {
                        Update::Event(change) => {
                            let dropped = watch.queue.push(change);
                            if dropped > 0 && watch.queue.mode() == Mode::Every {
                                env.warn(&format!("{}: too many unprocessed changes, the oldest was dropped", watch.entity));
                            }
                        }
                        Update::Status { status, .. } => match status {
                            SubscriptionStatus::Interrupted => env.warn(&format!("Subscription to {} interrupted: the connection was lost", watch.entity)),
                            SubscriptionStatus::Restored => env.info(&format!("Subscription to {} restored", watch.entity)),
                            SubscriptionStatus::Lagged { dropped } => env.warn(&format!("{}: {dropped} change(s) were merged because the plugin fell behind", watch.entity)),
                            SubscriptionStatus::Rejected { code, message } => env.error(&format!("Home Assistant rejected the subscription to {}: {code}: {message}", watch.entity)),
                            _ => {}
                        },
                        _ => {}
                    }
                }
                if events.is_finished() {
                    watch.sub = Sub::Failed { retry_at: now + RESUBSCRIBE_DELAY };
                }
            }
            if let Some(change) = watch.queue.pop() {
                emissions.push(Emission::new(*node, change_outputs(&change)));
            }
        }
        let requests = std::mem::take(&mut self.requests);
        for mut request in requests {
            let outputs = match &mut request.call {
                Call::Get { entity, reply } => reply.poll().map(|r| get_state_outputs(entity, r.map_err(|e| e.to_string()))),
                Call::Service { reply } => reply.poll().map(|r| service_outputs(r.map_err(|e| e.to_string()))),
            };
            match outputs {
                Some(outputs) => emissions.push(Emission::new(request.node, outputs)),
                None => self.requests.push(request),
            }
        }
        emissions
    }

    fn log_connection_state(&mut self, env: &mut impl Env) {
        let Some(link) = &self.link else { return };
        let state = link.session.connection_state();
        if self.logged.as_ref() == Some(&state) {
            return;
        }
        match &state {
            ConnectionState::Connected => env.info("Connected to Home Assistant"),
            ConnectionState::Reconnecting { attempt } => env.warn(&format!("Not connected to Home Assistant, reconnecting (attempt {attempt})")),
            ConnectionState::AuthFailed(message) => env.error(&format!("Home Assistant rejected the access token: {message}")),
            ConnectionState::Stopped => env.info("Home Assistant connection stopped"),
            _ => {}
        }
        self.logged = Some(state);
    }

    #[cfg(test)]
    pub fn connection_state(&self) -> Option<ConnectionState> {
        self.link.as_ref().map(|l| l.session.connection_state())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use ticknode_plugin_sdk::DataValue;
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
        calls: Vec<Value>,
    }

    impl Rig {
        fn new() -> Self {
            let mut rig = Rig { hub: Hub::default(), env: Fake::default(), now: Instant::now(), ws: 0, triggers: HashMap::new(), service_error: None, calls: Vec::new() };
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
                            let entity = request["trigger"]["entity_id"].as_str().unwrap().to_string();
                            self.triggers.insert(entity, id);
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
                                None => ok(Value::Null),
                                Some(message) => json!({"id": id, "type": "result", "success": false, "error": {"code": "home_assistant_error", "message": message}}),
                            });
                        }
                        _ => replies.push(ok(Value::Null)),
                    }
                }
                emissions.extend(self.recv(replies));
            }
            emissions
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
    fn requests_fail_promptly_when_not_connected() {
        let mut rig = Rig::new();
        // Configured but the socket is not up yet
        let outputs = rig.hub.get_state(node(9), "light.a", rig.now, &mut rig.env).expect("an immediate answer");
        assert!(has_exec(&outputs, GET_FAILED));
        assert_eq!(state_of(data(&outputs, GET_STATE)), "");
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
}
