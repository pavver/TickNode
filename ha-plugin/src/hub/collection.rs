//! Collects watched states, automation triggers and one-shot replies in order.
use super::requests::action_result;
use super::{EntityWatch, Env, Hub, Link, Watch};
use crate::triggers::outputs as trigger_outputs;
use ha_core_client::StateChange;
use std::time::Instant;
use ticknode_plugin_sdk::{Emission, Output};
impl Hub {
    pub(super) fn collect(&mut self, now: Instant, env: &mut impl Env) -> Vec<Emission> {
        let mut emissions = Vec::new();
        self.collect_watches(now, env, &mut emissions);
        self.collect_triggers(now, env, &mut emissions);
        self.collect_requests(&mut emissions);
        emissions
    }
    fn collect_watches(&mut self, now: Instant, env: &mut impl Env, emissions: &mut Vec<Emission>) {
        for (node, watch) in &mut self.watches {
            let change = watch.collect(now, &watch.config.clone(), false, env);
            if let Some(reply) = self.entities.get_mut(node) {
                if let Some(outputs) =
                    entity_outputs(watch, reply, change, &mut self.link, now, env)
                {
                    emissions.push(Emission::new(*node, outputs));
                }
            }
        }
    }
    fn collect_triggers(
        &mut self,
        now: Instant,
        env: &mut impl Env,
        emissions: &mut Vec<Emission>,
    ) {
        for (node, watch) in &mut self.triggers {
            let Some(config) = watch.config.clone() else {
                continue;
            };
            if let Some(fired) = watch.collect(now, config.platform.type_name(), true, env) {
                emissions.push(Emission::new(*node, trigger_outputs(&config, &fired)));
            }
        }
    }
    fn collect_requests(&mut self, emissions: &mut Vec<Emission>) {
        let requests = std::mem::take(&mut self.requests);
        for mut request in requests {
            let outputs = request.reply.poll().map(action_result);
            match outputs {
                Some(outputs) => emissions.push(Emission::new(request.node, outputs)),
                None => self.requests.push(request),
            }
        }
    }
}
fn entity_outputs(
    watch: &mut Watch<StateChange, String>,
    entity: &mut EntityWatch,
    change: Option<StateChange>,
    link: &mut Option<Link>,
    now: Instant,
    env: &mut impl Env,
) -> Option<Vec<Output>> {
    if let Some(change) = change {
        // A newer event wins over an outstanding baseline request.
        entity.reply = None;
        watch.snapshot_due = false;
        entity.check_units(
            [change.new_state.as_ref(), change.old_state.as_ref()],
            &watch.config,
            env,
        );
        return Some(crate::entity::change_outputs(
            &change,
            entity.value_type,
            entity.fire_on,
        ));
    }

    if watch.snapshot_due {
        watch.snapshot_due = false;
        if let Some(link) = link {
            entity.reply = Some(link.session.get_state(now, &watch.config));
        }
    }
    if let Some(pending) = &mut entity.reply
        && let Some(result) = pending.poll()
    {
        let output = match result {
            Ok(state) => {
                entity.check_units([Some(&state)], &watch.config, env);
                Some(crate::entity::outputs(Some(&state), entity.value_type))
            }
            Err(e) => {
                env.warn(&format!("Could not read entity {}: {e}", watch.config));
                None
            }
        };
        entity.reply = None;
        return output;
    }
    None
}

impl EntityWatch {
    fn check_units<'a>(
        &mut self,
        states: impl IntoIterator<Item = Option<&'a ha_core_client::State>>,
        entity_id: &str,
        env: &mut impl Env,
    ) {
        let mismatch = states
            .into_iter()
            .flatten()
            .any(|state| crate::entity::unit_mismatch(state, self.value_type));
        if mismatch && !self.unit_warning {
            env.warn(&format!(
                "Entity {entity_id}: missing or incompatible unit_of_measurement for {}; clearing the affected value pin",
                self.value_type.as_str()
            ));
        }
        self.unit_warning = mismatch;
    }
}
