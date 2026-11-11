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
mod timezone;

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
    /// Bumped whenever an entity's cached state is replaced (events and snapshots).
    revisions: HashMap<String, u64>,
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
    temperature_unit: Option<String>,
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
    /// Changes whenever the entity's cached state is replaced, even with an identical timestamp.
    pub fn revision(&self, entity: &str) -> u64 {
        self.revisions.get(entity).copied().unwrap_or(0)
    }
    pub fn temperature_unit(&self) -> Option<&str> {
        self.temperature_unit.as_deref()
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
        for id in self.states.keys() {
            *self.revisions.entry(id.clone()).or_default() += 1;
        }
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
        *self.revisions.entry(change.entity_id.clone()).or_default() += 1;
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
            self.temperature_unit = None;
            self.config = None;
            self.config_due = None;
            return true;
        }
        let refresh = match self.subscribe_changes(now, session, env) {
            Ok(refresh) => refresh,
            Err(()) => return true,
        };
        let invalid = self.collect_changes(now, env);
        let refresh = refresh || invalid;
        self.tick_offset(now, session, env);
        self.tick_config(now, session);

        refresh
    }
    fn collect_changes(&mut self, now: Instant, env: &mut impl Env) -> bool {
        let mut updates = StateUpdates {
            changes: Vec::new(),
            subscribed: self.subscribed,
            invalid: false,
        };
        if let Stream::Active(stream) = &mut self.changes {
            for update in stream.drain() {
                updates.collect(update, env);
            }
            if stream.is_finished() {
                self.changes = Stream::Idle;
                updates.subscribed = false;
                self.retry = Some(now + Duration::from_secs(30));
                updates.invalid = true;
            }
        }
        self.subscribed = updates.subscribed;
        if updates.invalid {
            self.invalidate();
        }
        for change in updates.changes {
            if self.snapshot_pending {
                self.overlay
                    .insert(change.entity_id.clone(), change.clone());
            }
            self.apply(change);
        }
        updates.invalid
    }
    fn subscribe_changes(
        &mut self,
        now: Instant,
        session: &mut Session,
        env: &mut impl Env,
    ) -> Result<bool, ()> {
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
                    return Err(());
                }
            }
        }
        Ok(refresh)
    }
}

struct StateUpdates {
    changes: Vec<StateChange>,
    subscribed: bool,
    invalid: bool,
}
impl StateUpdates {
    fn collect(&mut self, update: Update<StateChange>, env: &mut impl Env) {
        match update {
            Update::Event(change) => self.changes.push(change),
            Update::Status {
                status: SubscriptionStatus::Restored,
                ..
            } => {
                self.subscribed = true;
                self.invalid = true;
            }
            Update::Status {
                status: SubscriptionStatus::Interrupted,
                ..
            } => {
                self.subscribed = false;
                self.invalid = true;
            }
            Update::Status {
                status: SubscriptionStatus::Lagged { .. },
                ..
            } => {
                env.warn("Live state cache fell behind; requesting a fresh snapshot");
                self.invalid = true;
            }
            _ => {}
        }
    }
}
