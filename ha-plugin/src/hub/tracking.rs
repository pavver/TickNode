//! Shared entity subscriptions, node ownership and expiring runtime accesses.
use super::{Env, Hub, RESUBSCRIBE_DELAY, Sub, Watch};
use crate::queue::ChangeQueue;
use ha_core_client::{Reply, Source, State, StateChange};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use ticknode_plugin_sdk::NodeId;

pub(super) const TRACK_GRACE: Duration = Duration::from_secs(60);
pub(super) const LAZY_TTL: Duration = Duration::from_secs(10 * 60);

pub(super) struct Tracked {
    owners: HashSet<NodeId>,
    lazy_until: Option<Instant>,
    idle_since: Option<Instant>,
    watch: Watch<StateChange, String>,
    baseline: Option<Reply<State>>,
    baseline_due: Option<Instant>,
    waiting: bool,
    held: ChangeQueue<StateChange>,
}
impl Tracked {
    fn new(id: &str) -> Self {
        Self {
            owners: HashSet::new(),
            lazy_until: None,
            idle_since: None,
            watch: Watch {
                serial: 0,
                config: id.into(),
                queue: ChangeQueue::new(),
                sub: Sub::Idle,
                errors: Vec::new(),
                snapshot_due: false,
            },
            baseline: None,
            baseline_due: None,
            waiting: true,
            held: ChangeQueue::new(),
        }
    }
}
impl Hub {
    pub fn set_condition_entities(
        &mut self,
        node: NodeId,
        serial: u64,
        kind: crate::conditions::Kind,
        properties: &std::collections::BTreeMap<String, String>,
        now: Instant,
    ) {
        if self.conditions.get(&node) != Some(&serial) {
            return;
        }
        let ids: Vec<&str> = match kind {
            crate::conditions::Kind::Zone => ["entity_id", "zone"]
                .into_iter()
                .filter_map(|key| properties.get(key).map(String::as_str))
                .collect(),
            crate::conditions::Kind::Sun => {
                if ["after", "before"].into_iter().any(|key| {
                    properties
                        .get(key)
                        .is_some_and(|v| !v.is_empty() && v != "none")
                }) {
                    vec!["sun.sun"]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        self.track(node, ids, now);
    }
    pub(super) fn touch_action_target(&mut self, owner: NodeId, id: &str, now: Instant) {
        let entry = self
            .tracked
            .entry(id.into())
            .or_insert_with(|| Tracked::new(id));
        if !entry.owners.contains(&owner) {
            entry.lazy_until = Some(now + LAZY_TTL);
            entry.idle_since = None;
        }
    }
    /// Replaces all static entity dependencies of this owner.
    pub fn track<'a>(
        &mut self,
        owner: NodeId,
        ids: impl IntoIterator<Item = &'a str>,
        now: Instant,
    ) {
        self.untrack(owner, now);
        for id in ids.into_iter().map(str::trim).filter(|id| !id.is_empty()) {
            let entry = self
                .tracked
                .entry(id.into())
                .or_insert_with(|| Tracked::new(id));
            entry.owners.insert(owner);
            entry.idle_since = None;
            self.live.owned.insert(id.into());
        }
    }
    pub fn untrack(&mut self, owner: NodeId, now: Instant) {
        for (id, entry) in &mut self.tracked {
            if entry.owners.remove(&owner) && entry.owners.is_empty() {
                self.live.owned.remove(id);
                entry.idle_since.get_or_insert(now);
            }
        }
    }
    pub(super) fn reset_tracking(&mut self) {
        for (id, entry) in &mut self.tracked {
            entry.watch.sub = Sub::Idle;
            entry.watch.queue.clear();
            entry.watch.snapshot_due = false;
            entry.baseline = None;
            entry.baseline_due = None;
            entry.waiting = true;
            entry.held.clear();
            if !entry.owners.is_empty() {
                self.live.owned.insert(id.clone());
            }
        }
    }
    pub(super) fn tick_tracking(&mut self, now: Instant, env: &mut impl Env) {
        self.live.now = Some(now);
        for (id, at) in std::mem::take(&mut *self.live.lazy.borrow_mut()) {
            let entry = self
                .tracked
                .entry(id.clone())
                .or_insert_with(|| Tracked::new(&id));
            entry.lazy_until = Some(at + LAZY_TTL);
            entry.idle_since = None;
        }
        let expired: Vec<_> = self
            .tracked
            .iter_mut()
            .filter_map(|(id, entry)| {
                if entry.owners.is_empty() && entry.lazy_until.is_none_or(|due| now >= due) {
                    // Expired runtime owners receive the same grace period as static owners.
                    let since = *entry
                        .idle_since
                        .get_or_insert(entry.lazy_until.unwrap_or(now));
                    if now >= since + TRACK_GRACE {
                        return Some(id.clone());
                    }
                } else {
                    entry.idle_since = None;
                }
                None
            })
            .collect();
        for id in expired {
            if let Some(entry) = self.tracked.remove(&id) {
                self.cancel(entry.watch.sub, now);
            }
            self.live.forget(&id);
        }
        let Some(link) = &mut self.link else { return };
        for (id, entry) in &mut self.tracked {
            entry.watch.ensure(now, &mut link.session, |id| {
                Source::entity(id).without_merging()
            });
            let change = entry.watch.collect(now, id, false, env);
            if !link.session.is_connected() {
                entry.baseline = None;
                entry.baseline_due = None;
                entry.waiting = true;
                entry.watch.snapshot_due = false;
                entry.held.clear();
                for watch in self.watches.values_mut().filter(|w| w.config == *id) {
                    watch.queue.clear();
                }
                continue;
            }
            if entry.watch.snapshot_due {
                entry.watch.snapshot_due = false;
                entry.waiting = true;
                entry.baseline = None;
                entry.baseline_due = Some(now);
                self.live.invalidate(id);
                entry.held.clear();
                for watch in self.watches.values_mut().filter(|w| w.config == *id) {
                    watch.queue.clear();
                }
            }
            for change in change
                .into_iter()
                .chain(std::iter::from_fn(|| entry.watch.queue.pop()))
            {
                if entry.held.push(change) > 0 {
                    env.warn("Tracked entity updates fell behind; oldest change dropped");
                }
            }
            if entry.waiting
                && entry.baseline.is_none()
                && matches!(entry.watch.sub, Sub::Active(_))
                && entry.baseline_due.is_some_and(|due| now >= due)
            {
                entry.baseline = Some(link.session.get_state(now, id));
            }
            if let Some(result) = entry.baseline.as_mut().and_then(|r| r.poll()) {
                entry.baseline = None;
                match result {
                    Ok(state) => {
                        self.live.put(id, Some(state));
                        entry.waiting = false;
                        for watch in self.watches.values_mut().filter(|w| w.config == *id) {
                            watch.snapshot_due = true;
                        }
                    }
                    Err(error) => {
                        env.warn(&format!("Could not read tracked entity {id}: {error}"));
                        entry.baseline_due = Some(now + RESUBSCRIBE_DELAY);
                    }
                }
            }
            if !entry.waiting {
                while let Some(change) = entry.held.pop() {
                    if self.live.get(id).is_some_and(|s| {
                        change
                            .new_state
                            .as_ref()
                            .or(change.old_state.as_ref())
                            .is_some_and(|next| next.last_updated < s.last_updated)
                    }) {
                        continue;
                    }
                    self.live.put(id, change.new_state.clone());
                    for watch in self.watches.values_mut().filter(|w| w.config == *id) {
                        if watch.queue.push(change.clone()) > 0 {
                            env.warn("Entity node updates fell behind; oldest change dropped");
                        }
                    }
                }
            }
        }
    }
}
