//! Asynchronous condition template requests and first-render collection.

use super::{Env, Hub, Sub, TemplateCall};
use ha_core_client::{Source, SubscriptionStatus, TemplateUpdate, Update};
use std::time::{Duration, Instant};
use ticknode_plugin_sdk::{Emission, NodeId, Output};

impl Hub {
    pub fn add_condition(&mut self, node: NodeId) -> u64 {
        self.next_serial += 1;
        self.conditions.insert(node, self.next_serial);
        self.next_serial
    }

    pub fn cancel_templates(
        &mut self,
        node: NodeId,
        serial: u64,
        now: Instant,
        env: &mut impl Env,
    ) {
        for call in std::mem::take(&mut self.templates) {
            if call.node == node && call.serial == serial {
                self.cancel(call.sub, now);
            } else {
                self.templates.push(call);
            }
        }
        self.flush(now, env);
    }

    pub fn remove_condition(
        &mut self,
        node: NodeId,
        serial: u64,
        now: Instant,
        env: &mut impl Env,
    ) {
        self.cancel_templates(node, serial, now, env);
        if self.conditions.get(&node) == Some(&serial) {
            self.conditions.remove(&node);
        }
    }

    pub fn render_condition(
        &mut self,
        node: NodeId,
        serial: u64,
        template: &str,
        now: Instant,
        env: &mut impl Env,
    ) -> Vec<Output> {
        if template.is_empty()
            || self.templates.len() >= 64
            || self.link.as_ref().is_none_or(|l| !l.session.is_connected())
        {
            env.warn("Template condition cannot render: empty template, disconnected, or too many pending requests; returning No");
            return crate::conditions::outputs(false);
        }
        let sub = self
            .link
            .as_mut()
            .unwrap()
            .session
            .subscribe(now, Source::template(template));
        self.templates.push(TemplateCall {
            node,
            serial,
            sub: Sub::Pending(sub),
            deadline: now + Duration::from_secs(30),
        });
        self.flush(now, env);
        Vec::new()
    }

    pub(super) fn collect_templates(&mut self, now: Instant, env: &mut impl Env) -> Vec<Emission> {
        let mut emissions = Vec::new();
        for mut call in std::mem::take(&mut self.templates) {
            if self.conditions.get(&call.node) != Some(&call.serial) {
                self.cancel(call.sub, now);
                continue;
            }
            let mut result: Option<Result<bool, String>> = None;
            if self.link.as_ref().is_none_or(|l| !l.session.is_connected()) {
                result = Some(Err("Home Assistant disconnected".into()));
            } else if now >= call.deadline {
                result = Some(Err("Template render timed out".into()));
            }
            if result.is_none() {
                result = call.poll();
            }
            if let Some(result) = result {
                let value = match result {
                    Ok(value) => value,
                    Err(e) => {
                        env.warn(&format!(
                            "Template condition {}: {e}; returning No",
                            call.node.0
                        ));
                        false
                    }
                };
                emissions.push(Emission::new(call.node, crate::conditions::outputs(value)));
                self.cancel(call.sub, now);
            } else {
                self.templates.push(call);
            }
        }
        emissions
    }
}

impl TemplateCall {
    fn poll(&mut self) -> Option<Result<bool, String>> {
        let mut result = None;
        if result.is_none()
            && let Sub::Pending(p) = &mut self.sub
            && let Some(reply) = p.poll()
        {
            match reply {
                Ok(events) => self.sub = Sub::Active(events),
                Err(e) => result = Some(Err(e.to_string())),
            }
        }
        if result.is_none()
            && let Sub::Active(events) = &mut self.sub
        {
            for update in events.drain() {
                if let Some(value) = template_result(update) {
                    result = Some(value);
                    break;
                }
            }
            if result.is_none() && events.is_finished() {
                result = Some(Err("Template subscription ended".into()));
            }
        }
        result
    }
}

fn template_result(
    update: Update<TemplateUpdate<serde_json::Value>>,
) -> Option<Result<bool, String>> {
    match update {
        Update::Event(TemplateUpdate::Rendered(value)) => {
            Some(Ok(crate::conditions::template_bool(&value)))
        }
        Update::Event(error) => Some(Err(format!("Template render failed: {error:?}"))),
        Update::Status {
            status: SubscriptionStatus::Interrupted | SubscriptionStatus::Lagged { .. },
            ..
        } => Some(Err(
            "Template subscription was interrupted or lost events".into()
        )),
        _ => None,
    }
}
