//! Engine references share the same tracker as nodes and lazy accesses.
use super::{Env, Hub};
use std::time::Instant;
use ticknode_shared::{NodeId, Uuid};

// Real graph nodes use random UUIDs; reserve a non-node UUID for namespace ownership.
const ENGINE_VALUES: NodeId = NodeId(Uuid::from_u128(u128::MAX));
impl Hub {
    pub fn watch_values(
        &mut self,
        added: Vec<String>,
        removed: Vec<String>,
        now: Instant,
        env: &mut impl Env,
    ) {
        for key in removed {
            self.value_watches.remove(&key);
            self.published_values.remove(&key);
            env.set_value(&key, None);
        }
        self.value_watches.extend(added);
        let ids: Vec<_> = self.value_watches.iter().cloned().collect();
        self.track(ENGINE_VALUES, ids.iter().map(String::as_str), now);
        self.publish_values(env);
    }
    pub(super) fn publish_value_catalog(&self, env: &mut impl Env) {
        env.set_value_catalog(
            &self
                .catalog
                .iter()
                .map(crate::values::catalog)
                .collect::<Vec<_>>(),
        );
    }
    pub(super) fn publish_values(&mut self, env: &mut impl Env) {
        let connected = self.link.as_ref().is_some_and(|l| l.session.is_connected());
        for id in &self.value_watches {
            let value = self
                .live
                .get(id)
                .filter(|_| connected)
                .map(|state| crate::values::tree(state, self.live.temperature_unit()).0);
            if self.published_values.get(id) != Some(&value) {
                env.set_value(id, value.as_ref());
                self.published_values.insert(id.clone(), value);
            }
        }
    }
    pub(super) fn clear_values(&mut self, env: &mut impl Env) {
        for key in &self.value_watches {
            env.set_value(key, None);
        }
        self.published_values.clear();
        env.set_value_catalog(&[]);
    }
}

pub(super) fn publish_tracked(
    watched: &std::collections::BTreeSet<String>,
    published: &mut std::collections::BTreeMap<String, Option<ticknode_shared::DataValue>>,
    id: &str,
    state: Option<&ha_core_client::State>,
    temperature: Option<&str>,
    env: &mut impl Env,
) {
    if watched.contains(id) {
        let value = state.map(|state| crate::values::tree(state, temperature).0);
        env.set_value(id, value.as_ref());
        published.insert(id.into(), value);
    }
}
