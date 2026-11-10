//! One-shot action requests and action result conversion.

use super::{Env, Hub, InFlight, NOT_CONFIGURED};
use std::time::Instant;
use ticknode_plugin_sdk::{NodeId, Output};

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
        data: serde_json::Value,
        response: bool,
        now: Instant,
        env: &mut impl Env,
    ) -> Option<Vec<Output>> {
        let Some(link) = &mut self.link else {
            return Some(crate::actions::outputs(Err(NOT_CONFIGURED.into())));
        };
        let response = self
            .services
            .get(&format!("{domain}.{service}"))
            .map_or(response, |s| s.response);
        let mut request = serde_json::json!({"type":"call_service", "domain":domain, "service":service, "service_data":data});
        if !entity.is_empty() {
            request["target"] = serde_json::json!({"entity_id":[entity]});
        }
        if response {
            request["return_response"] = serde_json::Value::Bool(true);
        }
        let mut reply = match link.session.command(now, request) {
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
