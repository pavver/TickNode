//! Shared live state cache and Home Assistant's local UTC offset (WASI has no local timezone).
use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::FixedOffset;
use ha_core_client::{
    PendingSubscription, Reply, Session, SessionEvents, Source, State, SubscriptionStatus,
    TemplateUpdate, Update,
};
use serde_json::Value;

use crate::hub::Env;
mod timezone;

#[derive(Default)]
enum Stream<T> {
    #[default]
    Idle,
    Pending(PendingSubscription<T>),
    Active(SessionEvents<T>),
}

#[derive(Default)]
pub struct Live {
    states: HashMap<String, State>,
    /// Bumped whenever an entity's cached state is replaced (events and snapshots).
    revisions: HashMap<String, u64>,
    /// Runtime accesses are collected without requiring mutable condition evaluation.
    pub(super) lazy: std::cell::RefCell<HashMap<String, Instant>>,
    pub(super) owned: std::collections::HashSet<String>,
    pub(super) now: Option<Instant>,
    offset_stream: Stream<TemplateUpdate<Value>>,
    offset: Option<FixedOffset>,
    config: Option<Reply<Box<serde_json::value::RawValue>>>,
    config_due: Option<Instant>,
    zone: Option<String>,
    temperature_unit: Option<String>,
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
    pub fn state(&self, entity: &str) -> Result<&State, String> {
        self.touch(entity);
        let state = self
            .states
            .get(entity)
            .ok_or_else(|| format!("Entity {entity} state is not loaded yet"))?;
        if matches!(state.state.as_str(), "unknown" | "unavailable") {
            return Err(format!("Entity {entity} is {}", state.state));
        }
        Ok(state)
    }
    pub fn touch(&self, entity: &str) {
        if !entity.is_empty() && !self.owned.contains(entity) {
            self.lazy
                .borrow_mut()
                .insert(entity.to_string(), self.now.unwrap_or_else(Instant::now));
        }
    }
    pub fn invalidate(&mut self, entity: &str) {
        self.states.remove(entity);
        self.generation += 1;
    }
    pub fn forget(&mut self, entity: &str) {
        self.invalidate(entity);
        self.revisions.remove(entity);
    }
    pub fn put(&mut self, entity: &str, state: Option<State>) {
        *self.revisions.entry(entity.to_string()).or_default() += 1;
        if let Some(state) = state {
            self.states.insert(entity.to_string(), state);
        } else {
            self.states.remove(entity);
        }
    }
    #[cfg(test)]
    pub fn snapshot(&mut self, states: &[State]) {
        self.states.clear();
        for state in states {
            self.put(&state.entity_id, Some(state.clone()));
        }
    }
    pub fn offset(&self) -> Result<FixedOffset, String> {
        self.offset
            .filter(|_| self.zone.is_some())
            .ok_or_else(|| "Home Assistant timezone is not available".into())
    }
    pub fn tick(&mut self, now: Instant, session: &mut Session, env: &mut impl Env) {
        self.now = Some(now);
        if !session.is_connected() {
            if !self.states.is_empty() {
                self.generation += 1;
            }
            self.states.clear();
            self.offset = None;
            self.zone = None;
            self.temperature_unit = None;
            self.config = None;
            self.config_due = None;
            return;
        }
        self.tick_offset(now, session, env);
        self.tick_config(now, session);
    }
}
