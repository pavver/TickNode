//! Node-owned Telegram file lookup and download into host blob storage.
use crate::{
    api,
    hub::{Env, Hub},
    runtime, schema,
    validation::{self, Properties},
};
use serde_json::json;
use ticknode_plugin_sdk::*;

const MAX_DOWNLOAD_BYTES: u64 = 20 * 1024 * 1024;

struct PendingDownload {
    task: TaskId,
    request: HttpRequest,
    downloading: bool,
    waiting: bool,
    media: DataValue,
    generation: u64,
    retries: u8,
}

pub struct DownloadNode {
    props: Properties,
    pending: Option<PendingDownload>,
    error: Option<UiError>,
}

impl DownloadNode {
    pub fn new() -> Self {
        Self {
            props: schema::defaults("download"),
            pending: None,
            error: None,
        }
    }

    fn failed(&mut self, mut error: UiError, token: &str) -> Vec<Output> {
        if !token.is_empty() {
            error.fallback = error.fallback.replace(token, "[redacted]");
            for (_, value) in &mut error.args {
                *value = value.replace(token, "[redacted]");
            }
        }
        self.error = Some(error.clone());
        vec![
            Output::Data(PinId(12), error.fallback.into()),
            Output::Exec(PinId(11)),
            Output::Clear(PinId(13)),
        ]
    }

    fn completed(media: DataValue) -> Vec<Output> {
        vec![
            Output::Clear(PinId(12)),
            Output::Data(PinId(13), media),
            Output::Exec(PinId(10)),
        ]
    }

    fn start(
        &mut self,
        inputs: &Inputs,
        hub: &Hub,
        env: &mut impl Env,
    ) -> Result<Vec<Output>, UiError> {
        if self.pending.is_some() {
            return Err(validation::error(
                "busy",
                "This Telegram node already has an action in flight",
            ));
        }
        validation::stored("download", &self.props)?;
        let media = inputs.get(PinId(1)).ok_or_else(media_error)?;
        let DataValue::Object {
            type_name: Some(name),
            ..
        } = media
        else {
            return Err(media_error());
        };
        if !matches!(
            name.as_str(),
            "media.photo" | "media.audio" | "media.video" | "media.document"
        ) {
            return Err(media_error());
        }
        let kind = self.props.get("kind").map(String::as_str).unwrap_or("auto");
        if kind != "auto" && name != &format!("media.{kind}") {
            return Err(validation::error(
                "download-kind",
                "Media type does not match the selected Kind",
            ));
        }
        let Some(file_id) = nonempty_field(media, "file_id") else {
            if nonempty_field(media, "blob").is_some() {
                return Ok(Self::completed(media.clone()));
            }
            if nonempty_field(media, "url").is_some() {
                return Err(validation::error(
                    "download-url",
                    "Only Telegram files can be downloaded; use Download media (media/fetch) for URLs",
                ));
            }
            return Err(media_error());
        };
        hub.ready()?;
        let request = api::request(
            &hub.base,
            &hub.token,
            "getFile",
            json!({"file_id": file_id}).into(),
        );
        let task = env.http(request.clone()).map_err(|_| http_error())?;
        self.pending = Some(PendingDownload {
            task,
            request,
            downloading: false,
            waiting: false,
            media: media.clone(),
            generation: hub.generation,
            retries: 0,
        });
        Ok(Vec::new())
    }

    fn task(
        &mut self,
        id: TaskId,
        result: Result<TaskOutput, String>,
        hub: &mut Hub,
        env: &mut impl Env,
    ) -> Result<Vec<Output>, UiError> {
        if !self.pending.as_ref().is_some_and(|p| p.task == id) {
            return Ok(Vec::new());
        }
        let Some(mut pending) = self.pending.take() else {
            return Ok(Vec::new());
        };
        if pending.generation != hub.generation {
            return Err(validation::error(
                "changed",
                "Telegram settings changed while the action was pending; run it again",
            ));
        }
        if pending.waiting {
            if !matches!(result, Ok(TaskOutput::Delay)) {
                return Err(http_error());
            }
            hub.ready()?;
            pending.task = env
                .http(pending.request.clone())
                .map_err(|_| http_error())?;
            pending.waiting = false;
            self.pending = Some(pending);
            return Ok(Vec::new());
        }
        if pending.downloading
            && let Ok(TaskOutput::Blob(response)) = &result
            && (200..300).contains(&response.status)
        {
            let DataValue::Object { fields, .. } = &mut pending.media else {
                return Err(media_error());
            };
            insert_field(fields, "blob", response.blob.clone().into());
            if !fields.contains_key("size") {
                insert_field(fields, "size", (response.size as i64).into());
            }
            if nonempty_field(&pending.media, "mime_type").is_none()
                && let Some(content_type) = &response.content_type
                && let DataValue::Object { fields, .. } = &mut pending.media
            {
                insert_field(fields, "mime_type", content_type.clone().into());
            }
            return Ok(Self::completed(pending.media));
        }
        match api::response(result) {
            Ok(file) if !pending.downloading => {
                if file["file_size"].as_u64().unwrap_or(0) > MAX_DOWNLOAD_BYTES {
                    return Err(validation::error(
                        "download-size",
                        "Telegram file exceeds download limit of 20 MB",
                    ));
                }
                let path = file["file_path"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(file_response_error)?;
                // Keep the credential-bearing URL only in the private pending request.
                pending.request =
                    HttpRequest::get(format!("{}/file/bot{}/{path}", hub.base, hub.token))
                        .save_to_blob(true);
                pending.request.timeout_ms = 60_000;
                pending.task = env
                    .http(pending.request.clone())
                    .map_err(|_| http_error())?;
                pending.downloading = true;
                self.pending = Some(pending);
                Ok(Vec::new())
            }
            Ok(_) => Err(file_response_error()),
            Err(failure) => {
                if failure.code == 401 {
                    hub.unauthorized();
                }
                if failure.code == 429 && pending.retries < 5 {
                    pending.task = env
                        .delay(failure.retry_after.unwrap_or(1).max(1))
                        .map_err(|_| http_error())?;
                    pending.waiting = true;
                    pending.retries += 1;
                    self.pending = Some(pending);
                    return Ok(Vec::new());
                }
                Err(failure.ui())
            }
        }
    }

    fn run(
        &mut self,
        trigger: Trigger,
        inputs: &Inputs,
        hub: &mut Hub,
        env: &mut impl Env,
    ) -> Vec<Output> {
        self.error = None;
        let result = match trigger {
            Trigger::Pin(_) => self.start(inputs, hub, env),
            Trigger::Task { id, result } => self.task(id, result, hub, env),
        };
        result.unwrap_or_else(|error| self.failed(error, &hub.token))
    }
}

fn nonempty_field<'a>(media: &'a DataValue, key: &str) -> Option<&'a str> {
    field_string(media, key).filter(|value| !value.trim().is_empty())
}

fn insert_field(
    fields: &mut std::collections::BTreeMap<String, ObjectField>,
    key: &str,
    value: DataValue,
) {
    fields.insert(
        key.into(),
        ObjectField {
            name: key.into(),
            description: String::new(),
            value,
        },
    );
}

fn media_error() -> UiError {
    validation::error("media", "A media value must have url, file_id, or blob set")
}

fn file_response_error() -> UiError {
    validation::error(
        "download-response",
        "Telegram returned an invalid file response",
    )
}

fn http_error() -> UiError {
    validation::error("http", "Could not start the Telegram HTTP request")
}

impl PluginNode for DownloadNode {
    fn definition(&self) -> NodeDefinition {
        schema::action_definition("download", &self.props)
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        self.props
            .iter()
            .map(|(name, value)| (name.clone(), value.clone().into()))
            .collect()
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiError> {
        let DataValue::String(text) = value else {
            return Err(validation::error("property", "Expected a text property"));
        };
        validation::property("download", name, text, &self.props)
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if let DataValue::String(text) = value
            && self.props.contains_key(name)
        {
            self.props.insert(name.into(), text);
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        let outputs = runtime::with(|hub, env| self.run(trigger, inputs, hub, env));
        if let Some(error) = &self.error {
            runtime::report_error(error, true);
        }
        outputs
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contract_tests;
