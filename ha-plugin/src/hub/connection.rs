//! Session configuration, transport driving and per-tick emission collection.

use std::time::Instant;
use ha_core_client::{Action, ClientConfig, ConnectionState, Session};
use ticknode_plugin_sdk::{Emission, WsEvent};
use crate::triggers::outputs as trigger_outputs;
use super::{Call, Env, Hub, Link, Sub, change_outputs, get_state_outputs, service_outputs};
use super::requests::action_result;

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
    pub(super) fn flush(&mut self, now: Instant, env: &mut impl Env) {
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
