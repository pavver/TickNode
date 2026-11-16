//! Entity/service catalog refresh and dynamic option publishing.

use super::{ENTITIES_RETRY, Env, Hub, SERVICES_REFRESH};
use crate::entities::option_sources;
use ha_core_client::ConnectionState;
use std::collections::BTreeSet;
use std::time::Instant;
use ticknode_plugin_sdk::NodeId;

impl Hub {
    pub fn metadata(&self, id: &str) -> Option<crate::entities::CatalogEntry> {
        self.live
            .get(id)
            .map(crate::entities::CatalogEntry::from)
            .or_else(|| self.catalog.get(*self.catalog_index.get(id)?).cloned())
    }

    /// Reads picker metadata after connect and debounced registry changes.
    pub(super) fn refresh_entities(&mut self, now: Instant, env: &mut impl Env) {
        let Some(link) = &self.link else { return };
        if link.session.connection_state() != ConnectionState::Connected {
            // The answer to a request made on a lost connection never comes; ask again after the reconnect
            self.entities_reply = None;
            self.entities_due = None;
            self.catalog_loaded = false;
            return;
        }
        if let Some(reply) = &mut self.entities_reply {
            match reply.poll() {
                None => return,
                Some(Ok(states)) => {
                    self.publish_entities(&states, env);
                    self.catalog_loaded = true;
                }
                Some(Err(e)) => {
                    env.warn(&format!("Could not read the list of entities: {e}"));
                    self.entities_due = Some(now + ENTITIES_RETRY);
                }
            }
            self.entities_reply = None;
        }
        if self.entities_due.is_some_and(|due| now >= due)
            || (!self.catalog_loaded && self.entities_due.is_none())
        {
            self.entities_due = None;
            if let Some(link) = &mut self.link {
                self.entities_reply = Some(link.session.get_states(now));
            }
        }
    }

    fn publish_entities(&mut self, states: &[ha_core_client::State], env: &mut impl Env) {
        self.catalog = states
            .iter()
            .map(crate::entities::CatalogEntry::from)
            .collect();
        self.catalog_index = self
            .catalog
            .iter()
            .enumerate()
            .map(|(i, e)| (e.entity_id.clone(), i))
            .collect();
        for (source, options) in option_sources(&self.catalog) {
            env.set_options(source, &options);
            if source == "entities" {
                self.entity_options = options;
            }
        }
        self.publish_action_targets(env);
        self.publish_action_sources(env);
    }

    pub(super) fn refresh_services(&mut self, now: Instant, env: &mut impl Env) {
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
                    self.services_due = Some(now + SERVICES_REFRESH);
                }
                Some(Err(e)) => {
                    env.warn(&format!("Could not read the list of actions: {e}"));
                    self.services_due = Some(now + ENTITIES_RETRY);
                }
            }
            self.services_reply = None;
        }
        if self.services_due.is_none_or(|due| now >= due)
            && let Some(link) = &mut self.link
        {
            self.services_reply = Some(link.session.get_services(now));
        }
    }

    /// Publishes actionable entities for `ha/action` (`"action_targets"`).
    fn publish_action_targets(&mut self, env: &mut impl Env) {
        let options = crate::entities::action_target_options(&self.catalog, &self.services);
        env.set_options(crate::entities::ACTION_TARGETS_SOURCE, &options);
    }

    /// Publishes all actions for input mode (`"actions"`) and per-entity action lists (`"actions:<entity>"`).
    fn publish_action_sources(&mut self, env: &mut impl Env) {
        if self.services.is_empty() {
            return;
        }
        env.set_options(
            "actions",
            &crate::actions::options_for(&self.services, None),
        );
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
            let state = self.metadata(&entity_id);
            let options = crate::actions::options_for(
                &self.services,
                Some((
                    &entity_id,
                    state
                        .as_ref()
                        .map(|s| s as &dyn crate::entities::EntityMetadata),
                )),
            );
            env.set_options(&source, &options);
            sources.insert(source);
        }
        for old in self.domain_sources.difference(&sources) {
            env.set_options(old, &[]);
        }
        self.domain_sources = sources;
    }

    pub fn set_action_target(
        &mut self,
        node: NodeId,
        target: &str,
        now: Instant,
        env: &mut impl Env,
    ) {
        let target = target.trim();
        self.track(
            node,
            crate::actions::entity_ids(target).unwrap_or_default(),
            now,
        );
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
            let state = self.metadata(target);
            let options = crate::actions::options_for(
                &self.services,
                Some((
                    target,
                    state
                        .as_ref()
                        .map(|s| s as &dyn crate::entities::EntityMetadata),
                )),
            );
            env.set_options(&source, &options);
        }
    }

    pub fn remove_action_target(&mut self, node: NodeId, now: Instant, env: &mut impl Env) {
        self.untrack(node, now);
        if let Some(target) = self.action_targets.remove(&node) {
            let source = crate::actions::action_source(&target);
            if !self.action_targets.values().any(|t| t == &target)
                && !self
                    .entity_options
                    .iter()
                    .take(192)
                    .any(|o| o.value == target)
            {
                self.domain_sources.remove(&source);
                env.set_options(&source, &[]);
            }
        }
    }
}
