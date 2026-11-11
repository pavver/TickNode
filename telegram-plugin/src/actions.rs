//! Node-owned HTTP actions. Telegram rate limits wait through a node-owned delay task.
mod body;
#[cfg(test)]
mod tests;
use crate::{
    api,
    hub::{Env, Hub},
    model::Message,
    runtime, schema,
    validation::{self, Properties},
};
use serde_json::Value;
use ticknode_plugin_sdk::*;

struct Pending {
    task: TaskId,
    waiting: bool,
    method: &'static str,
    body: Value,
    generation: u64,
    retries: u8,
}
pub struct ActionNode {
    kind: String,
    props: Properties,
    pending: Option<Pending>,
    error: Option<UiError>,
}
impl ActionNode {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            props: schema::defaults(kind),
            pending: None,
            error: None,
        }
    }
    fn failed(&mut self, error: UiError) -> Vec<Output> {
        self.error = Some(error.clone());
        let mut outputs = vec![
            Output::Data(PinId(12), error.fallback.into()),
            Output::Exec(PinId(11)),
        ];
        if self.kind == "send" {
            outputs.extend([Output::Clear(PinId(13)), Output::Clear(PinId(14))]);
        }
        outputs
    }
    fn run(
        &mut self,
        trigger: Trigger,
        inputs: &Inputs,
        now: u64,
        hub: &mut Hub,
        env: &mut impl Env,
    ) -> Vec<Output> {
        self.error = None;
        match trigger {
            Trigger::Pin(_) => {
                if self.pending.is_some() {
                    return self.failed(validation::error(
                        "busy",
                        "This Telegram node already has an action in flight",
                    ));
                }
                if let Err(error) = hub
                    .ready()
                    .and_then(|_| validation::stored(&self.kind, &self.props))
                {
                    return self.failed(error);
                }
                let (method, body) = match body::build(&self.kind, &self.props, inputs, hub) {
                    Ok(request) => request,
                    Err(error) => return self.failed(error),
                };
                let chat = body["chat_id"].as_i64().unwrap();
                let task = match env.http(api::request(&hub.base, &hub.token, method, body.clone()))
                {
                    Ok(id) => id,
                    Err(_) => {
                        return self.failed(validation::error(
                            "http",
                            "Could not start the Telegram HTTP request",
                        ));
                    }
                };
                if self.kind == "send" {
                    hub.lookup(chat, now);
                }
                self.pending = Some(Pending {
                    task,
                    waiting: false,
                    method,
                    body,
                    generation: hub.generation,
                    retries: 0,
                });
                Vec::new()
            }
            Trigger::Task { id, result } => {
                if !self.pending.as_ref().is_some_and(|p| p.task == id) {
                    return Vec::new();
                }
                let mut pending = self.pending.take().unwrap();
                if pending.generation != hub.generation {
                    return self.failed(validation::error(
                        "changed",
                        "Telegram settings changed while the action was pending; run it again",
                    ));
                }
                if pending.waiting {
                    if !matches!(result, Ok(TaskOutput::Delay)) {
                        return self.failed(validation::error(
                            "http",
                            "Could not start the Telegram HTTP request",
                        ));
                    }
                    if let Err(error) = hub.ready() {
                        return self.failed(error);
                    }
                    match env.http(api::request(
                        &hub.base,
                        &hub.token,
                        pending.method,
                        pending.body.clone(),
                    )) {
                        Ok(task) => {
                            pending.task = task;
                            pending.waiting = false;
                            self.pending = Some(pending);
                            return Vec::new();
                        }
                        Err(_) => {
                            return self.failed(validation::error(
                                "http",
                                "Could not start the Telegram HTTP request",
                            ));
                        }
                    }
                }
                match api::response(result) {
                    Ok(value) => {
                        let mut outputs = vec![Output::Clear(PinId(12))];
                        if self.kind == "send" {
                            let Ok(message) = serde_json::from_value::<Message>(value) else {
                                return self.failed(validation::error(
                                    "response",
                                    "Telegram returned an invalid message",
                                ));
                            };
                            hub.directory.message(&message, now);
                            hub.remember(&message);
                            outputs.extend([
                                Output::Data(PinId(13), message.value()),
                                Output::Data(PinId(14), message.message_id.into()),
                            ]);
                        }
                        outputs.push(Output::Exec(PinId(10)));
                        outputs
                    }
                    Err(mut failure) => {
                        failure.description = failure.description.replace(&hub.token, "[redacted]");
                        if failure.code == 401 {
                            hub.unauthorized();
                        }
                        if failure.code == 429 && pending.retries < 5 {
                            match env.delay(failure.retry_after.unwrap_or(1).max(1)) {
                                Ok(task) => {
                                    pending.task = task;
                                    pending.waiting = true;
                                    pending.retries += 1;
                                    self.pending = Some(pending);
                                    return Vec::new();
                                }
                                Err(_) => {}
                            }
                        }
                        self.failed(failure.ui())
                    }
                }
            }
        }
    }
}
impl PluginNode for ActionNode {
    fn definition(&self) -> NodeDefinition {
        schema::definition(&self.kind)
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        schema::definition(&self.kind)
            .properties
            .into_iter()
            .map(|p| {
                let value = self.props.get(&p.name).cloned().unwrap_or_default();
                (p.name, value.into())
            })
            .collect()
    }
    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        let DataValue::String(text) = value else {
            return Err(validation::error("property", "Expected a text property"));
        };
        validation::property(&self.kind, name, text, &self.props)
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::String(text) = value
            && self.props.contains_key(name)
        {
            self.props.insert(name.into(), text);
        }
    }
    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let outputs = runtime::with(|hub, env| self.run(trigger, inputs, runtime::now(), hub, env));
        if let Some(error) = &self.error {
            host::node_error(error, true);
        }
        outputs
    }
}
