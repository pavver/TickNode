//! Plugin-level state: the single Home Assistant connection (a sans-IO `Session`), the per-node
//! subscriptions and the in-flight requests. It talks to the outside world only through [`Env`], and the
//! clock is passed in, so the whole logic runs in tests on the host target.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use ha_core_client::{
    Action, ClientConfig, ConnectionState, PendingSubscription, Reply, Session, Source, State, StateChange, Target,
    Update, SubscriptionStatus, TriggerFired, TemplateUpdate,
};
use ticknode_plugin_sdk::{Emission, NodeId, Output, PinId, PropertyOption, WsEvent, WsId};

use crate::convert::state_value;
use crate::entities::option_sources;
use crate::triggers::{TriggerConfig, outputs as trigger_outputs};
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
/// How often the entity list for the picker is refreshed while connected.
const ENTITIES_REFRESH: Duration = Duration::from_secs(5 * 60);
/// Wait before asking for the entity list again after a failed request.
const ENTITIES_RETRY: Duration = Duration::from_secs(30);

/// What the hub needs from the host.
pub trait Env {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String>;
    fn ws_send(&mut self, id: WsId, text: &str) -> Result<(), String>;
    fn ws_close(&mut self, id: WsId);
    fn info(&mut self, message: &str);
    fn warn(&mut self, message: &str);
    fn error(&mut self, message: &str);
    /// Publishes the options of a dynamic property source (`host::set_options`).
    fn set_options(&mut self, source: &str, options: &[PropertyOption]);
}

struct Link {
    url: String,
    session: Session,
    ws: Option<WsId>,
}

enum Sub<T> {
    /// Not subscribed (no entity yet, or a new one was just set).
    Idle,
    Pending(PendingSubscription<T>),
    Active(ha_core_client::SessionEvents<T>),
    /// The subscription failed or ended; try again at `retry_at`.
    Failed { retry_at: Instant },
}

struct Watch<T, C> {
    /// Distinguishes a node from an older one with the same id, so a late `Drop` cannot remove the new one.
    serial: u64,
    config: C,
    queue: ChangeQueue<T>,
    sub: Sub<T>,
    /// Errors delivered from the node's poll, where host logging has the node context.
    errors: Vec<String>,
    /// Initial subscription or restored connection needs a fresh baseline for entity data nodes.
    snapshot_due: bool,
}

impl<T, C> Watch<T, C> {
    fn ensure(&mut self, now: Instant, session: &mut Session, source: impl FnOnce(&C) -> Source<T>) {
        let due = match self.sub {
            Sub::Idle => true,
            Sub::Failed { retry_at } => now >= retry_at,
            Sub::Pending(_) | Sub::Active(_) => false,
        };
        if due {
            self.sub = Sub::Pending(session.subscribe(now, source(&self.config)));
        }
    }

    /// Common pending/active/retry lifecycle and bounded, ordered delivery for all sources.
    fn collect(&mut self, now: Instant, label: &str, node_errors: bool, env: &mut impl Env) -> Option<T> {
        if let Sub::Pending(pending) = &mut self.sub {
            match pending.poll() {
                None => {}
                Some(Ok(events)) => { self.sub = Sub::Active(events); self.snapshot_due = true; },
                Some(Err(e)) => {
                    let message = format!("Subscribing to {label} failed: {e}");
                    if node_errors {
                        self.errors.push(message);
                    } else {
                        env.warn(&message);
                    }
                    self.sub = Sub::Failed { retry_at: now + RESUBSCRIBE_DELAY };
                }
            }
        }
        if let Sub::Active(events) = &mut self.sub {
            for update in events.drain() {
                match update {
                    Update::Event(change) => {
                        let dropped = self.queue.push(change);
                        if dropped > 0 && self.queue.mode() == Mode::Every {
                            env.warn(&format!("{label}: too many unprocessed changes, the oldest was dropped"));
                        }
                    }
                    Update::Status { status, .. } => match status {
                        SubscriptionStatus::Interrupted => {
                            env.warn(&format!("Subscription to {label} interrupted: the connection was lost"))
                        }
                        SubscriptionStatus::Restored => { self.snapshot_due = true; env.info(&format!("Subscription to {label} restored")); },
                        SubscriptionStatus::Lagged { dropped } => env.warn(&format!(
                            "{label}: {dropped} change(s) were {} because the plugin fell behind",
                            if node_errors { "dropped" } else { "merged" }
                        )),
                        SubscriptionStatus::Rejected { code, message } => {
                            let message = format!("Home Assistant rejected the subscription to {label}: {code}: {message}");
                            if node_errors {
                                self.errors.push(message);
                            } else {
                                env.error(&message);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            if events.is_finished() {
                self.sub = Sub::Failed { retry_at: now + RESUBSCRIBE_DELAY };
            }
        }
        self.queue.pop()
    }
}

enum Call {
    Get { reply: Reply<State> },
    Service { reply: Reply<()> },
    Action { reply: Reply<Box<serde_json::value::RawValue>> },
}

struct InFlight {
    node: NodeId,
    call: Call,
}

struct TemplateCall {
    node: NodeId,
    serial: u64,
    sub: Sub<TemplateUpdate<serde_json::Value>>,
    deadline: Instant,
}

#[derive(Default)]
pub struct Hub {
    pub live: crate::live::Live,
    conditions: HashMap<NodeId, u64>,
    templates: Vec<TemplateCall>,
    link: Option<Link>,
    watches: HashMap<NodeId, Watch<StateChange, String>>,
    triggers: HashMap<NodeId, Watch<TriggerFired, Option<TriggerConfig>>>,
    requests: Vec<InFlight>,
    next_serial: u64,
    /// The last connection state that was logged.
    logged: Option<ConnectionState>,
    /// The in-flight `get_states` request that feeds the entity picker.
    entities_reply: Option<Reply<Vec<State>>>,
    /// When the entity list is asked for next. `None`: as soon as the connection is up.
    entities_due: Option<Instant>,
    services_reply: Option<Reply<serde_json::Value>>,
    services_due: Option<Instant>,
    pub services: BTreeMap<String, crate::actions::Service>,
    entity_options: Vec<PropertyOption>,
    domain_sources: BTreeSet<String>,
    action_targets: HashMap<NodeId, String>,
    state_nodes: HashMap<NodeId, String>,
    state_sources: BTreeSet<String>,
    /// Entity data nodes share the watch lifecycle and additionally read a baseline after (re)subscribe.
    entities: HashMap<NodeId, Option<Reply<State>>>,
}

/// Outputs of a `ha/state_changed` pulse. Missing states clear their output pins.
pub fn change_outputs(change: &StateChange) -> Vec<Output> {
    let value = |pin, state: &Option<State>| state.as_ref().map_or(Output::Clear(pin), |s| Output::Data(pin, state_value(s)));
    vec![value(NEW, &change.new_state), value(OLD, &change.old_state), Output::Exec(CHANGED)]
}

pub fn get_state_outputs(result: Result<State, String>) -> Vec<Output> {
    match result {
        Ok(state) => vec![Output::Data(GET_STATE, state_value(&state)), Output::Data(GET_ERROR, "".into()), Output::Exec(GET_DONE)],
        Err(error) => vec![Output::Clear(GET_STATE), Output::Data(GET_ERROR, error.into()), Output::Exec(GET_FAILED)],
    }
}

pub fn service_outputs(result: Result<(), String>) -> Vec<Output> {
    match result {
        Ok(()) => vec![Output::Data(CALL_ERROR, "".into()), Output::Exec(CALL_DONE)],
        Err(error) => vec![Output::Data(CALL_ERROR, error.into()), Output::Exec(CALL_FAILED)],
    }
}

fn action_result(result: ha_core_client::Result<Box<serde_json::value::RawValue>>) -> Vec<Output> {
    crate::actions::outputs(result.map_err(|e| e.to_string()).and_then(|raw| serde_json::from_str(raw.get()).map_err(|e| e.to_string())))
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
            watch.snapshot_due = false;
        }
        for watch in self.triggers.values_mut() {
            watch.sub = Sub::Idle;
            watch.queue.clear();
            watch.errors.clear();
        }
        self.logged = None;
        self.live.reset();
        self.entities_reply = None;
        self.entities_due = None;
        self.services_due = None;
        self.services_reply = None;
        self.services.clear();
        self.entity_options.clear();
        self.action_targets.clear();
        self.state_nodes.clear();
        env.set_options("actions", &[]);
        env.set_options(crate::entities::ACTION_TARGETS_SOURCE, &[]);
        env.set_options(crate::states::STATES_SOURCE, &[]);
        env.set_options(crate::states::ATTRIBUTES_SOURCE, &[]);
        for source in std::mem::take(&mut self.domain_sources) { env.set_options(&source, &[]); }
        for source in std::mem::take(&mut self.state_sources) { env.set_options(&source, &[]); }
        for reply in self.entities.values_mut() { *reply = None; }
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
        self.watches.insert(node, Watch { serial, config: String::new(), queue: ChangeQueue::new(mode), sub: Sub::Idle, errors: Vec::new(), snapshot_due: false });
        serial
    }

    pub fn add_entity(&mut self, node: NodeId) -> u64 {
        let serial = self.add_watch(node, Mode::Latest);
        self.entities.insert(node, None);
        serial
    }

    /// Unregisters the node and cancels its subscription.
    pub fn remove_watch(&mut self, node: NodeId, serial: u64, now: Instant, env: &mut impl Env) {
        if self.watches.get(&node).is_some_and(|w| w.serial == serial) {
            self.entities.remove(&node);
            if let Some(watch) = self.watches.remove(&node) {
                self.cancel(watch.sub, now);
            }
            self.flush(now, env);
        }
    }

    pub fn set_entity(&mut self, node: NodeId, serial: u64, entity: &str, now: Instant, env: &mut impl Env) {
        let Some(watch) = self.watches.get_mut(&node).filter(|w| w.serial == serial) else { return };
        if watch.config == entity {
            return;
        }
        watch.config = entity.to_string();
        watch.queue.clear();
        watch.snapshot_due = false;
        if let Some(reply) = self.entities.get_mut(&node) { *reply = None; }
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

    fn cancel<T>(&mut self, sub: Sub<T>, now: Instant) {
        let events = match sub {
            Sub::Active(events) => Some(events),
            // Offline subscriptions can already be accepted before the hub polls the reply.
            // SessionEvents does not unsubscribe on drop, so consume a ready reply first.
            Sub::Pending(mut pending) => pending.poll().and_then(Result::ok),
            Sub::Idle | Sub::Failed { .. } => None,
        };
        if let (Some(events), Some(link)) = (events, &mut self.link) {
            // The reply is not needed: the session cleans up on its own.
            let _ = link.session.unsubscribe(now, events.subscription());
        }
        // An unresolved pending subscription is abandoned by dropping it.
    }

    /// Registers an automation trigger; it remains idle until its configuration is complete.
    pub fn add_trigger(&mut self, node: NodeId, now: Instant) -> u64 {
        self.next_serial += 1;
        let serial = self.next_serial;
        if let Some(old) = self.triggers.insert(node, Watch { serial, config: None, queue: ChangeQueue::new(Mode::Every), sub: Sub::Idle, errors: Vec::new(), snapshot_due: false }) {
            self.cancel(old.sub, now);
        }
        serial
    }

    pub fn set_trigger(&mut self, node: NodeId, serial: u64, config: Option<TriggerConfig>, now: Instant, env: &mut impl Env) {
        let Some(watch) = self.triggers.get_mut(&node).filter(|w| w.serial == serial) else { return };
        if watch.config == config { return; }
        watch.config = config;
        watch.queue.clear();
        watch.errors.clear();
        let old = std::mem::replace(&mut watch.sub, Sub::Idle);
        self.cancel(old, now);
        self.ensure_subscriptions(now);
        self.flush(now, env);
    }

    pub fn remove_trigger(&mut self, node: NodeId, serial: u64, now: Instant, env: &mut impl Env) {
        if self.triggers.get(&node).is_some_and(|w| w.serial == serial) {
            if let Some(watch) = self.triggers.remove(&node) { self.cancel(watch.sub, now); }
            self.flush(now, env);
        }
    }

    pub fn take_trigger_errors(&mut self, node: NodeId, serial: u64) -> Vec<String> {
        self.triggers.get_mut(&node).filter(|w| w.serial == serial)
            .map(|w| std::mem::take(&mut w.errors)).unwrap_or_default()
    }

    // --- Requests ---

    pub fn add_condition(&mut self, node: NodeId) -> u64 {
        self.next_serial += 1;
        self.conditions.insert(node, self.next_serial);
        self.next_serial
    }

    pub fn cancel_templates(&mut self, node: NodeId, serial: u64, now: Instant, env: &mut impl Env) {
        for call in std::mem::take(&mut self.templates) {
            if call.node == node && call.serial == serial { self.cancel(call.sub, now); }
            else { self.templates.push(call); }
        }
        self.flush(now, env);
    }

    pub fn remove_condition(&mut self, node: NodeId, serial: u64, now: Instant, env: &mut impl Env) {
        self.cancel_templates(node, serial, now, env);
        if self.conditions.get(&node) == Some(&serial) { self.conditions.remove(&node); }
    }

    pub fn render_condition(&mut self, node: NodeId, serial: u64, template: &str, now: Instant, env: &mut impl Env) -> Vec<Output> {
        if template.is_empty() || self.templates.len() >= 64 || self.link.as_ref().is_none_or(|l| !l.session.is_connected()) {
            env.warn("Template condition cannot render: empty template, disconnected, or too many pending requests; returning No");
            return crate::conditions::outputs(false);
        }
        let sub = self.link.as_mut().unwrap().session.subscribe(now, Source::template(template));
        self.templates.push(TemplateCall { node, serial, sub: Sub::Pending(sub), deadline: now + Duration::from_secs(30) });
        self.flush(now, env);
        Vec::new()
    }

    fn collect_templates(&mut self, now: Instant, env: &mut impl Env) -> Vec<Emission> {
        let mut emissions = Vec::new();
        for mut call in std::mem::take(&mut self.templates) {
            if self.conditions.get(&call.node) != Some(&call.serial) { self.cancel(call.sub, now); continue; }
            let mut result: Option<Result<bool, String>> = None;
            if self.link.as_ref().is_none_or(|l| !l.session.is_connected()) {
                result = Some(Err("Home Assistant disconnected".into()));
            } else if now >= call.deadline { result = Some(Err("Template render timed out".into())); }
            if result.is_none() && let Sub::Pending(p) = &mut call.sub && let Some(reply) = p.poll() {
                match reply {
                    Ok(events) => call.sub = Sub::Active(events),
                    Err(e) => result = Some(Err(e.to_string())),
                }
            }
            if result.is_none() && let Sub::Active(events) = &mut call.sub {
                for update in events.drain() {
                    match update {
                        Update::Event(TemplateUpdate::Rendered(value)) => { result = Some(Ok(crate::conditions::template_bool(&value))); break; }
                        Update::Event(error) => { result = Some(Err(format!("Template render failed: {error:?}"))); break; }
                        Update::Status { status: SubscriptionStatus::Interrupted | SubscriptionStatus::Lagged { .. }, .. } => {
                            result = Some(Err("Template subscription was interrupted or lost events".into())); break;
                        }
                        _ => {}
                    }
                }
                if result.is_none() && events.is_finished() { result = Some(Err("Template subscription ended".into())); }
            }
            if let Some(result) = result {
                let value = match result { Ok(value) => value, Err(e) => { env.warn(&format!("Template condition {}: {e}; returning No", call.node.0)); false } };
                emissions.push(Emission::new(call.node, crate::conditions::outputs(value)));
                self.cancel(call.sub, now);
            } else { self.templates.push(call); }
        }
        emissions
    }

    /// Starts reading a state. `Some(outputs)` when the result is already known (for example a failure
    /// while disconnected), otherwise the node gets its outputs from a later emission.
    pub fn get_state(&mut self, node: NodeId, entity: &str, now: Instant, env: &mut impl Env) -> Option<Vec<Output>> {
        let Some(link) = &mut self.link else {
            return Some(get_state_outputs(Err(NOT_CONFIGURED.into())));
        };
        let mut reply = link.session.get_state(now, entity);
        let first = reply.poll();
        let immediate = match first {
            Some(result) => Some(get_state_outputs(result.map_err(|e| e.to_string()))),
            None => {
                self.requests.push(InFlight { node, call: Call::Get { reply } });
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

    /// Session has no typed response-call helper, so use its one-shot command API.
    pub fn action(
        &mut self, node: NodeId, domain: &str, service: &str, entity: &str,
        data: serde_json::Value, response: bool, now: Instant, env: &mut impl Env,
    ) -> Option<Vec<Output>> {
        let Some(link) = &mut self.link else { return Some(crate::actions::outputs(Err(NOT_CONFIGURED.into()))) };
        let response = self.services.get(&format!("{domain}.{service}")).map_or(response, |s| s.response);
        let mut request = serde_json::json!({"type":"call_service", "domain":domain, "service":service, "service_data":data});
        if !entity.is_empty() { request["target"] = serde_json::json!({"entity_id":[entity]}); }
        if response { request["return_response"] = serde_json::Value::Bool(true); }
        let mut reply = match link.session.command(now, request) {
            Ok(reply) => reply,
            Err(e) => return Some(crate::actions::outputs(Err(e.to_string()))),
        };
        let result = match reply.poll() {
            Some(result) => Some(action_result(result)),
            None => { self.requests.push(InFlight { node, call: Call::Action { reply } }); None }
        };
        self.flush(now, env);
        result
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
        if let Some(link) = &mut self.link && self.live.tick(now, &mut link.session, env) {
            self.entities_reply = None;
            self.entities_due = None;
        }
        let mut emissions = self.collect(now, env);
        emissions.extend(self.collect_templates(now, env));
        self.refresh_entities(now, env);
        self.refresh_services(now, env);
        self.flush(now, env);
        self.log_connection_state(env);
        emissions
    }

    /// Keeps the entity picker's options fresh: asks for all states once the connection is up (also after
    /// every reconnect) and then every [`ENTITIES_REFRESH`]; publishes the answer through [`Env::set_options`].
    fn refresh_entities(&mut self, now: Instant, env: &mut impl Env) {
        let Some(link) = &self.link else { return };
        if link.session.connection_state() != ConnectionState::Connected || !self.live.active() {
            // The answer to a request made on a lost connection never comes; ask again after the reconnect
            self.entities_reply = None;
            self.entities_due = None;
            return;
        }
        if let Some(reply) = &mut self.entities_reply {
            match reply.poll() {
                None => return,
                Some(Ok(states)) => {
                    self.live.snapshot(&states);
                    for (source, options) in option_sources(&states) {
                        env.set_options(source, &options);
                        if source == "entities" { self.entity_options = options; }
                    }
                    self.publish_action_targets(env);
                    self.publish_action_sources(env);
                    self.publish_state_sources(env);
                    self.entities_due = Some(now + ENTITIES_REFRESH);
                }
                Some(Err(e)) => {
                    env.warn(&format!("Could not read the list of entities: {e}"));
                    self.entities_due = Some(now + ENTITIES_RETRY);
                }
            }
            self.entities_reply = None;
        }
        if self.entities_due.is_none_or(|due| now >= due) {
            self.live.begin_snapshot();
            self.entities_reply = Some(self.link.as_mut().expect("connected").session.get_states(now));
        }
    }

    fn refresh_services(&mut self, now: Instant, env: &mut impl Env) {
        let Some(link) = &self.link else { return };
        if link.session.connection_state() != ConnectionState::Connected {
            self.services_reply = None;
            self.services_due = None;
            return;
        }
        if let Some(reply) = &mut self.services_reply {
            match reply.poll() {
                None => return,
                Some(Ok(json)) => {
                    self.services = crate::actions::services(&json);
                    self.publish_action_targets(env);
                    self.publish_action_sources(env);
                    self.services_due = Some(now + ENTITIES_REFRESH);
                }
                Some(Err(e)) => {
                    env.warn(&format!("Could not read the list of actions: {e}"));
                    self.services_due = Some(now + ENTITIES_RETRY);
                }
            }
            self.services_reply = None;
        }
        if self.services_due.is_none_or(|due| now >= due) {
            self.services_reply = Some(self.link.as_mut().expect("connected").session.get_services(now));
        }
    }

    /// Publishes actionable entities for `ha/action` (`"action_targets"`).
    fn publish_action_targets(&mut self, env: &mut impl Env) {
        let states = self.live.all_states();
        if states.is_empty() {
            return;
        }
        let options = crate::entities::action_target_options(&states, &self.services);
        env.set_options(crate::entities::ACTION_TARGETS_SOURCE, &options);
    }

    /// Publishes target-less actions (`"actions"`) and per-entity action lists (`"actions:<entity>"`).
    fn publish_action_sources(&mut self, env: &mut impl Env) {
        if self.services.is_empty() {
            return;
        }
        env.set_options("actions", &crate::actions::options_for(&self.services, None));
        let mut targets: BTreeSet<String> = self
            .action_targets
            .values()
            .filter(|t| !t.trim().is_empty())
            .cloned()
            .collect();
        // Pre-publish actions for known entities up to the option sources limit (256 - ~16 overhead = 192).
        for opt in self.entity_options.iter().take(192) {
            targets.insert(opt.value.clone());
        }
        let mut sources = BTreeSet::new();
        for entity_id in targets {
            let source = crate::actions::action_source(&entity_id);
            let state = self.live.get(&entity_id);
            let options = crate::actions::options_for(&self.services, Some((&entity_id, state)));
            env.set_options(&source, &options);
            sources.insert(source);
        }
        for old in self.domain_sources.difference(&sources) {
            env.set_options(old, &[]);
        }
        self.domain_sources = sources;
    }

    pub fn set_action_target(&mut self, node: NodeId, target: &str, env: &mut impl Env) {
        let target = target.trim();
        if target.is_empty() {
            self.action_targets.remove(&node);
            return;
        }
        self.action_targets.insert(node, target.to_string());
        if self.services.is_empty() {
            return;
        }
        let source = crate::actions::action_source(target);
        if self.domain_sources.insert(source.clone()) {
            let state = self.live.get(target);
            let options = crate::actions::options_for(&self.services, Some((target, state)));
            env.set_options(&source, &options);
        }
    }

    pub fn remove_action_target(&mut self, node: NodeId, env: &mut impl Env) {
        if let Some(target) = self.action_targets.remove(&node) {
            let source = crate::actions::action_source(&target);
            if !self.action_targets.values().any(|t| t == &target)
                && !self.entity_options.iter().take(192).any(|o| o.value == target)
            {
                self.domain_sources.remove(&source);
                env.set_options(&source, &[]);
            }
        }
    }

    /// Publishes generic state/attribute fallback options and entity-specific options for active trigger/condition nodes.
    pub fn publish_state_sources(&mut self, env: &mut impl Env) {
        env.set_options(
            crate::states::STATES_SOURCE,
            &crate::states::state_options("", None, true),
        );
        env.set_options(
            crate::states::ATTRIBUTES_SOURCE,
            &crate::states::attribute_options("", None),
        );
        let mut sources = BTreeSet::new();
        for entity in self.state_nodes.values().filter(|e| !e.trim().is_empty()) {
            let state_src = crate::states::state_source(entity);
            let attr_src = crate::states::attribute_source(entity);
            let state = self.live.get(entity);
            env.set_options(&state_src, &crate::states::state_options(entity, state, true));
            env.set_options(&attr_src, &crate::states::attribute_options(entity, state));
            sources.insert(state_src);
            sources.insert(attr_src);
        }
        for old in self.state_sources.difference(&sources) {
            env.set_options(old, &[]);
        }
        self.state_sources = sources;
    }

    pub fn set_state_node_entity(&mut self, node: NodeId, entity: &str, env: &mut impl Env) {
        let entity = entity.trim();
        let old = self.state_nodes.insert(node, entity.to_string());
        if let Some(old_entity) = old {
            if !old_entity.is_empty() && old_entity != entity && !self.state_nodes.values().any(|e| e == &old_entity) {
                let old_state_src = crate::states::state_source(&old_entity);
                let old_attr_src = crate::states::attribute_source(&old_entity);
                self.state_sources.remove(&old_state_src);
                self.state_sources.remove(&old_attr_src);
                env.set_options(&old_state_src, &[]);
                env.set_options(&old_attr_src, &[]);
            }
        }
        if entity.is_empty() {
            self.state_nodes.remove(&node);
            env.set_options(
                crate::states::STATES_SOURCE,
                &crate::states::state_options("", None, true),
            );
            env.set_options(
                crate::states::ATTRIBUTES_SOURCE,
                &crate::states::attribute_options("", None),
            );
            return;
        }
        let state_src = crate::states::state_source(entity);
        let attr_src = crate::states::attribute_source(entity);
        let state = self.live.get(entity);
        self.state_sources.insert(state_src.clone());
        self.state_sources.insert(attr_src.clone());
        env.set_options(&state_src, &crate::states::state_options(entity, state, true));
        env.set_options(&attr_src, &crate::states::attribute_options(entity, state));
    }

    pub fn remove_state_node(&mut self, node: NodeId, env: &mut impl Env) {
        if let Some(entity) = self.state_nodes.remove(&node) {
            if !entity.is_empty() && !self.state_nodes.values().any(|e| e == &entity) {
                let state_src = crate::states::state_source(&entity);
                let attr_src = crate::states::attribute_source(&entity);
                self.state_sources.remove(&state_src);
                self.state_sources.remove(&attr_src);
                env.set_options(&state_src, &[]);
                env.set_options(&attr_src, &[]);
            }
        }
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
            if !watch.config.is_empty() {
                watch.ensure(now, &mut link.session, |entity| Source::entity(entity.as_str()).without_merging());
            }
        }
        for watch in self.triggers.values_mut() {
            if watch.config.is_some() {
                watch.ensure(now, &mut link.session, |config| Source::trigger(config.as_ref().expect("checked above").trigger.clone()));
            }
        }
    }

    fn collect(&mut self, now: Instant, env: &mut impl Env) -> Vec<Emission> {
        let mut emissions = Vec::new();
        for (node, watch) in &mut self.watches {
            let change = watch.collect(now, &watch.config.clone(), false, env);
            if let Some(reply) = self.entities.get_mut(node) {
                if let Some(change) = change {
                    // A newer event wins over an outstanding baseline request.
                    *reply = None;
                    watch.snapshot_due = false;
                    emissions.push(Emission::new(*node, crate::entity::outputs(change.new_state.as_ref())));
                } else {
                    if watch.snapshot_due {
                        watch.snapshot_due = false;
                        if let Some(link) = &mut self.link { *reply = Some(link.session.get_state(now, &watch.config)); }
                    }
                    if let Some(pending) = reply && let Some(result) = pending.poll() {
                        match result {
                            Ok(state) => emissions.push(Emission::new(*node, crate::entity::outputs(Some(&state)))),
                            Err(e) => env.warn(&format!("Could not read entity {}: {e}", watch.config)),
                        }
                        *reply = None;
                    }
                }
            } else if let Some(change) = change {
                emissions.push(Emission::new(*node, change_outputs(&change)));
            }
        }
        for (node, watch) in &mut self.triggers {
            let Some(config) = watch.config.clone() else { continue };
            if let Some(fired) = watch.collect(now, config.platform.type_name(), true, env) {
                emissions.push(Emission::new(*node, trigger_outputs(&config, &fired)));
            }
        }
        let requests = std::mem::take(&mut self.requests);
        for mut request in requests {
            let outputs = match &mut request.call {
                Call::Get { reply } => reply.poll().map(|r| get_state_outputs(r.map_err(|e| e.to_string()))),
                Call::Service { reply } => reply.poll().map(|r| service_outputs(r.map_err(|e| e.to_string()))),
                Call::Action { reply } => reply.poll().map(action_result),
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
    use crate::entities::ENTITIES_SOURCE;
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

}
