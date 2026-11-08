//! Collects watched states, automation triggers and one-shot replies in order.
use super::requests::action_result;
use super::{Call, Env, Hub, Link, Watch, change_outputs, get_state_outputs, service_outputs};
use crate::triggers::outputs as trigger_outputs;
use ha_core_client::{Reply, State, StateChange};
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
            } else if let Some(change) = change {
                emissions.push(Emission::new(*node, change_outputs(&change)));
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
            let outputs = match &mut request.call {
                Call::Get { reply } => reply
                    .poll()
                    .map(|r| get_state_outputs(r.map_err(|e| e.to_string()))),
                Call::Service { reply } => reply
                    .poll()
                    .map(|r| service_outputs(r.map_err(|e| e.to_string()))),
                Call::Action { reply } => reply.poll().map(action_result),
            };
            match outputs {
                Some(outputs) => emissions.push(Emission::new(request.node, outputs)),
                None => self.requests.push(request),
            }
        }
    }
}
fn entity_outputs(
    watch: &mut Watch<StateChange, String>,
    reply: &mut Option<Reply<State>>,
    change: Option<StateChange>,
    link: &mut Option<Link>,
    now: Instant,
    env: &mut impl Env,
) -> Option<Vec<Output>> {
    if let Some(change) = change {
        // A newer event wins over an outstanding baseline request.
        *reply = None;
        watch.snapshot_due = false;
        return Some(crate::entity::outputs(change.new_state.as_ref()));
    }

    if watch.snapshot_due {
        watch.snapshot_due = false;
        if let Some(link) = link {
            *reply = Some(link.session.get_state(now, &watch.config));
        }
    }
    if let Some(pending) = reply
        && let Some(result) = pending.poll()
    {
        let output = match result {
            Ok(state) => Some(crate::entity::outputs(Some(&state))),
            Err(e) => {
                env.warn(&format!("Could not read entity {}: {e}", watch.config));
                None
            }
        };
        *reply = None;
        return output;
    }
    None
}
