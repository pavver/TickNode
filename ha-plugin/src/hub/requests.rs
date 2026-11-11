//! One-shot action requests and action result conversion.

use super::{Env, Hub, InFlight, NOT_CONFIGURED};
use std::time::{Duration, Instant};
use ticknode_plugin_sdk::{NodeId, Output};

/// After a call is sent, the cached state of its targets is stale until HA reports an update. Skipping a
/// follow-up call against that stale state could lose it (turn_on sent, cache still "off", turn_off
/// skipped), so skipping waits for a newer state or this timeout.
const COMMAND_SETTLE: Duration = Duration::from_secs(10);

pub(super) struct Commanded {
    at: Instant,
    /// Cache revision of the target when the call was sent.
    seen: u64,
}

pub(super) fn action_result(
    result: ha_core_client::Result<Box<serde_json::value::RawValue>>,
) -> Vec<Output> {
    crate::actions::outputs(
        result
            .map_err(|e| e.to_string())
            .and_then(|raw| serde_json::from_str(raw.get()).map_err(|e| e.to_string())),
    )
}

impl Hub {
    /// Session has no typed response-call helper, so use its one-shot command API.
    pub fn action(
        &mut self,
        node: NodeId,
        domain: &str,
        service: &str,
        entity: &str,
        mut data: serde_json::Value,
        response: bool,
        skip_unchanged: bool,
        now: Instant,
        env: &mut impl Env,
    ) -> Option<Vec<Output>> {
        let Some(link) = &self.link else {
            return Some(crate::actions::outputs(Err(NOT_CONFIGURED.into())));
        };
        let service = match crate::actions::resolve_state(domain, service, &mut data) {
            Ok(service) => service,
            Err(error) => return Some(crate::actions::outputs(Err(error))),
        };
        let ids = crate::actions::entity_ids(entity).unwrap_or_default();
        let stamp = |live: &crate::live::Live, id: &str| live.revision(id);
        self.commanded.retain(|id, c| now < c.at + COMMAND_SETTLE && stamp(&self.live, id) == c.seen);
        let settled = ids.iter().all(|id| !self.commanded.contains_key(*id));
        if skip_unchanged
            && settled
            && link.session.is_connected()
            && crate::actions::unchanged(&self.live, domain, service, entity, &data)
        {
            env.info(&format!(
                "Skipped {domain}.{service}: all target entities already have the desired state"
            ));
            return Some(crate::actions::outputs(Ok(serde_json::Value::Null)));
        }
        let response = self
            .services
            .get(&format!("{domain}.{service}"))
            .map_or(response, |s| s.response);
        let mut request = serde_json::json!({"type":"call_service", "domain":domain, "service":service, "service_data":data});
        if !entity.is_empty() {
            let entities = crate::actions::entity_ids(entity).unwrap_or_else(|| vec![entity]);
            request["target"] = serde_json::json!({"entity_id":entities});
        }
        if response {
            request["return_response"] = serde_json::Value::Bool(true);
        }
        for id in &ids {
            let seen = stamp(&self.live, id);
            self.commanded.insert((*id).to_string(), Commanded { at: now, seen });
        }
        let mut reply = match self
            .link
            .as_mut()
            .expect("configured")
            .session
            .command(now, request)
        {
            Ok(reply) => reply,
            Err(e) => return Some(crate::actions::outputs(Err(e.to_string()))),
        };
        let result = match reply.poll() {
            Some(result) => Some(action_result(result)),
            None => {
                self.requests.push(InFlight { node, reply });
                None
            }
        };
        self.flush(now, env);
        result
    }
}
