//! Entity/service catalog refresh and dynamic option publishing.

use super::{ENTITIES_REFRESH, ENTITIES_RETRY, Env, Hub};
use crate::entities::option_sources;
use ha_core_client::ConnectionState;
use std::collections::BTreeSet;
use std::time::Instant;
use ticknode_plugin_sdk::NodeId;

impl Hub {
    /// Keeps the entity picker's options fresh: asks for all states once the connection is up (also after
    /// every reconnect) and then every [`ENTITIES_REFRESH`]; publishes the answer through [`Env::set_options`].
    pub(super) fn refresh_entities(&mut self, now: Instant, env: &mut impl Env) {
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
                    self.publish_entities(&states, env);
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
            self.entities_reply = Some(
                self.link
                    .as_mut()
                    .expect("connected")
                    .session
                    .get_states(now),
            );
        }
    }

    fn publish_entities(&mut self, states: &[ha_core_client::State], env: &mut impl Env) {
        self.live.snapshot(states);
        for (source, options) in option_sources(states) {
            env.set_options(source, &options);
            if source == "entities" {
                self.entity_options = options;
            }
        }
        self.publish_action_targets(env);
        self.publish_action_sources(env);
        self.publish_state_sources(env);
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
            self.services_reply = Some(
                self.link
                    .as_mut()
                    .expect("connected")
                    .session
                    .get_services(now),
            );
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
            env.set_options(
                &state_src,
                &crate::states::state_options(entity, state, true),
            );
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
            if !old_entity.is_empty()
                && old_entity != entity
                && !self.state_nodes.values().any(|e| e == &old_entity)
            {
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
        env.set_options(
            &state_src,
            &crate::states::state_options(entity, state, true),
        );
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
}
