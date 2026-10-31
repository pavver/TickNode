//! Shared live state cache and Home Assistant's local UTC offset (WASI has no local timezone).
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::FixedOffset;
use ha_core_client::{
    PendingSubscription, Reply, Session, SessionEvents, Source, State, StateChange,
    SubscriptionStatus, TemplateUpdate, Update,
};
use serde_json::Value;

use crate::hub::Env;

enum Stream<T> {
    Idle,
    Pending(PendingSubscription<T>),
    Active(SessionEvents<T>),
}
impl<T> Default for Stream<T> {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Default)]
pub struct Live {
    states: HashMap<String, State>,
    /// Retains changes received while a snapshot is in flight, including removals.
    overlay: HashMap<String, StateChange>,
    snapshot_pending: bool,
    ready: bool,
    subscribed: bool,
    changes: Stream<StateChange>,
    offset_stream: Stream<TemplateUpdate<Value>>,
    offset: Option<FixedOffset>,
    config: Option<Reply<Box<serde_json::value::RawValue>>>,
    config_due: Option<Instant>,
    zone: Option<String>,
    retry: Option<Instant>,
    offset_retry: Option<Instant>,
    pub generation: u64,
}

impl Live {
    #[cfg(test)]
    pub fn test_offset(&mut self, seconds: i32) {
        self.offset = FixedOffset::east_opt(seconds);
        self.zone = Some("Test".into());
    }
    pub fn reset(&mut self) {
        let generation = self.generation + 1;
        *self = Self::default();
        self.generation = generation;
    }
    fn invalidate(&mut self) {
        self.ready = false;
        self.states.clear();
        self.overlay.clear();
        self.snapshot_pending = false;
        self.generation += 1;
    }
    pub fn active(&self) -> bool {
        self.subscribed
    }
    pub fn get(&self, entity: &str) -> Option<&State> {
        self.states.get(entity)
    }
    pub fn all_states(&self) -> Vec<State> {
        self.states.values().cloned().collect()
    }
    pub fn state(&self, entity: &str) -> Result<&State, String> {
        if !self.ready {
            return Err("Home Assistant state cache is not ready or is disconnected".into());
        }
        let state = self
            .states
            .get(entity)
            .ok_or_else(|| format!("Entity {entity} is not known"))?;
        if matches!(state.state.as_str(), "unknown" | "unavailable") {
            return Err(format!("Entity {entity} is {}", state.state));
        }
        Ok(state)
    }
    pub fn offset(&self) -> Result<FixedOffset, String> {
        self.offset
            .filter(|_| self.zone.is_some())
            .ok_or_else(|| "Home Assistant timezone is not available".into())
    }
    pub fn begin_snapshot(&mut self) {
        self.overlay.clear();
        self.snapshot_pending = true;
    }
    pub fn snapshot(&mut self, states: &[State]) {
        self.states = states
            .iter()
            .map(|s| (s.entity_id.clone(), s.clone()))
            .collect();
        for (_, change) in std::mem::take(&mut self.overlay) {
            self.apply(change);
        }
        self.snapshot_pending = false;
        self.ready = true;
    }

    fn apply(&mut self, change: StateChange) {
        // A snapshot may itself be newer than an event that was queued before HA handled get_states.
        let timestamp = change
            .new_state
            .as_ref()
            .or(change.old_state.as_ref())
            .map(|s| s.last_updated);
        if let Some(current) = self.states.get(&change.entity_id)
            && timestamp.is_some_and(|at| at < current.last_updated)
        {
            return;
        }
        if let Some(state) = change.new_state {
            self.states.insert(change.entity_id, state);
        } else {
            self.states.remove(&change.entity_id);
        }
    }

    /// Returns true when the hub must discard an outstanding snapshot and request a new baseline.
    pub fn tick(&mut self, now: Instant, session: &mut Session, env: &mut impl Env) -> bool {
        if !session.is_connected() {
            self.subscribed = false;
            if self.ready || self.snapshot_pending {
                self.invalidate();
            }
            self.offset = None;
            self.zone = None;
            self.config = None;
            self.config_due = None;
            return true;
        }
        if matches!(self.changes, Stream::Idle) && self.retry.is_none_or(|due| now >= due) {
            self.changes = Stream::Pending(session.subscribe(now, Source::state_changes()));
        }
        let mut refresh = false;
        if let Stream::Pending(p) = &mut self.changes
            && let Some(result) = p.poll()
        {
            match result {
                Ok(stream) => {
                    self.changes = Stream::Active(stream);
                    self.subscribed = true;
                    refresh = true;
                }
                Err(e) => {
                    env.warn(&format!("Live state cache subscription failed: {e}"));
                    self.changes = Stream::Idle;
                    self.retry = Some(now + Duration::from_secs(30));
                    self.invalidate();
                    return true;
                }
            }
        }
        let mut invalid = false;
        let mut changes = Vec::new();
        if let Stream::Active(stream) = &mut self.changes {
            for update in stream.drain() {
                match update {
                    Update::Event(change) => {
                        changes.push(change);
                    }
                    Update::Status {
                        status: SubscriptionStatus::Restored,
                        ..
                    } => {
                        self.subscribed = true;
                        invalid = true;
                    }
                    Update::Status {
                        status: SubscriptionStatus::Interrupted,
                        ..
                    } => {
                        self.subscribed = false;
                        invalid = true;
                    }
                    Update::Status {
                        status: SubscriptionStatus::Lagged { .. },
                        ..
                    } => {
                        env.warn("Live state cache fell behind; requesting a fresh snapshot");
                        invalid = true;
                    }
                    _ => {}
                }
            }
            if stream.is_finished() {
                self.changes = Stream::Idle;
                self.subscribed = false;
                self.retry = Some(now + Duration::from_secs(30));
                invalid = true;
            }
        }
        if invalid {
            self.invalidate();
            refresh = true;
        }
        for change in changes {
            if self.snapshot_pending {
                self.overlay
                    .insert(change.entity_id.clone(), change.clone());
            }
            self.apply(change);
        }
        // HA's render_template tracks now() and refreshes on each minute, including DST changes.
        if matches!(self.offset_stream, Stream::Idle)
            && self.offset_retry.is_none_or(|due| now >= due)
        {
            self.offset_stream = Stream::Pending(session.subscribe(
                now,
                Source::template("{{ now().utcoffset().total_seconds() | int }}"),
            ));
        }
        if let Stream::Pending(p) = &mut self.offset_stream
            && let Some(result) = p.poll()
        {
            match result {
                Ok(stream) => self.offset_stream = Stream::Active(stream),
                Err(e) => {
                    env.warn(&format!(
                        "Home Assistant UTC offset subscription failed: {e}"
                    ));
                    self.offset_stream = Stream::Idle;
                    self.offset_retry = Some(now + Duration::from_secs(30));
                }
            }
        }
        if let Stream::Active(stream) = &mut self.offset_stream {
            for update in stream.drain() {
                match update {
                    Update::Event(TemplateUpdate::Rendered(value)) => {
                        self.offset = value
                            .as_i64()
                            .or_else(|| value.as_str()?.parse().ok())
                            .and_then(|n| i32::try_from(n).ok())
                            .and_then(FixedOffset::east_opt);
                    }
                    Update::Event(_)
                    | Update::Status {
                        status:
                            SubscriptionStatus::Interrupted
                            | SubscriptionStatus::Restored
                            | SubscriptionStatus::Lagged { .. },
                        ..
                    } => self.offset = None,
                    _ => {}
                }
            }
            if stream.is_finished() {
                self.offset_stream = Stream::Idle;
                self.offset = None;
                self.offset_retry = Some(now + Duration::from_secs(30));
            }
        }
        if let Some(reply) = &mut self.config
            && let Some(result) = reply.poll()
        {
            self.zone = result
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(raw.get()).ok())
                .and_then(|v| {
                    v["time_zone"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                });
            self.config = None;
            self.config_due =
                Some(now + Duration::from_secs(if self.zone.is_some() { 300 } else { 30 }));
        }
        if self.config.is_none() && self.config_due.is_none_or(|due| now >= due) {
            self.config = session
                .command(now, serde_json::json!({"type":"get_config"}))
                .ok();
        }
        refresh
    }
}
