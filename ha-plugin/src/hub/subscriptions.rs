//! Subscription registration, cancellation, retry and ordered event queues.

use std::time::Instant;
use ha_core_client::{Session, Source, SubscriptionStatus, Update};
use ticknode_plugin_sdk::NodeId;
use crate::queue::{ChangeQueue, Mode};
use crate::triggers::TriggerConfig;
use super::{Env, Hub, RESUBSCRIBE_DELAY, Sub, Watch};

impl<T, C> Watch<T, C> {
    pub(super) fn ensure(&mut self, now: Instant, session: &mut Session, source: impl FnOnce(&C) -> Source<T>) {
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
    pub(super) fn collect(&mut self, now: Instant, label: &str, node_errors: bool, env: &mut impl Env) -> Option<T> {
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

impl Hub {
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

    pub(super) fn cancel<T>(&mut self, sub: Sub<T>, now: Instant) {
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


    pub(super) fn ensure_subscriptions(&mut self, now: Instant) {
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

}
