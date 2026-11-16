//! Rare registry events invalidate the slim entity catalog after a debounce.
use super::{Env, Hub, Sub, Watch};
use crate::queue::ChangeQueue;
use ha_core_client::Source;
use std::time::{Duration, Instant};

pub(super) const REGISTRY_DEBOUNCE: Duration = Duration::from_secs(10);
impl Hub {
    pub(super) fn tick_registry(&mut self, now: Instant, env: &mut impl Env) {
        let Some(link) = &mut self.link else { return };
        let watch = self.registry.get_or_insert_with(|| Watch {
            serial: 0,
            config: (),
            queue: ChangeQueue::new(),
            sub: Sub::Idle,
            errors: Vec::new(),
            snapshot_due: false,
        });
        watch.ensure(now, &mut link.session, |_| {
            Source::events("entity_registry_updated")
        });
        let changed = watch.collect(now, "entity registry", false, env).is_some();
        let mut changed = changed;
        while watch.queue.pop().is_some() {
            changed = true;
        }
        if changed {
            self.entities_due = Some(now + REGISTRY_DEBOUNCE);
        }
        // Connection restoration already causes a catalog read; only data events debounce another.
        watch.snapshot_due = false;
    }
}
