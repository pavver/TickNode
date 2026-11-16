use crate::{Node, NodeContext, TaskId, TaskOutput, TaskResult};
use std::collections::BTreeMap;
use ticknode_shared::*;

/// Native active node downloading media from an HTTP(S) endpoint into blob storage.
pub struct MediaFetchNode {
    pub id: NodeId,
    pub kind: String,
}

impl MediaFetchNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            kind: "photo".into(),
        }
    }
}

impl Node for MediaFetchNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 50,
            properties: vec![PropertyDefinition {
                name: "kind".into(),
                title: "Kind".into(),
                description: "Media kind".into(),
                kind: PropertyKind::Select(vec![
                    ("photo".into(), "Photo".into()),
                    ("audio".into(), "Audio".into()),
                    ("video".into(), "Video".into()),
                    ("document".into(), "File".into()),
                ]),
                input: None,
            }],
            type_name: "media/fetch".into(),
            title: "Download media".into(),
            category: "Network".into(),
            description: "Downloads a media file into blob storage".into(),
            help: "Downloads a media file from an HTTP(S) URL into host blob storage and produces a media descriptor.\n\n• URL — HTTP(S) URL to download from. Basic authentication in the URL (http://user:pass@host/...) is supported.\n• Headers — optional HTTP headers, one 'Name: value' per line.\n• Kind — target media type (photo, audio, video, document).\n\nFires Done on success or Failed on error.".into(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "In".into(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "URL".into(),
                    pin_type: PinType::Data(DataType::String),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Headers".into(),
                    pin_type: PinType::Data(DataType::String),
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(10),
                    name: "Done".into(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(11),
                    name: "Failed".into(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(12),
                    name: "Error".into(),
                    pin_type: PinType::Data(DataType::String),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(13),
                    name: "Media".into(),
                    pin_type: PinType::Data(DataType::Named(format!("media.{}", self.kind))),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("kind".into(), DataValue::String(self.kind.clone()))]
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
        if name == "kind" && !valid_kind(value) {
            return Err(
                UiMessage::new("error-invalid-property", "Invalid property: kind")
                    .arg("property", "kind"),
            );
        }
        Ok(())
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "kind"
            && let DataValue::String(k) = value
            && valid_kind(&DataValue::String(k.clone()))
        {
            self.kind = k;
        }
    }

    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin != Some(PinId(0)) {
            return vec![];
        }
        match self.start_request(ctx) {
            Ok(()) => vec![],
            Err(message) => self.failed(message, ctx),
        }
    }

    fn on_task(
        &mut self,
        _task: TaskId,
        result: TaskResult,
        ctx: &mut NodeContext,
    ) -> Vec<NodeOutput> {
        match result {
            Ok(TaskOutput::Blob(response)) => vec![
                NodeOutput::Clear(PinId(12)),
                NodeOutput::Data(PinId(13), self.media_value(response, ctx)),
                NodeOutput::Execution(PinId(10)),
            ],
            Ok(TaskOutput::Http(response)) => self.failed(
                UiMessage::new(
                    "error-media-fetch-http",
                    format!("HTTP {}", response.status),
                )
                .arg("status", response.status.to_string()),
                ctx,
            ),
            Ok(TaskOutput::Delay) => vec![],
            Err(error) => self.failed(transport_error(error), ctx),
        }
    }
}

impl MediaFetchNode {
    fn start_request(&self, ctx: &mut NodeContext) -> Result<(), UiMessage> {
        let url = match ctx.get_value(PinId(1)) {
            Some(DataValue::String(url)) if !url.trim().is_empty() => url.trim().to_string(),
            _ => {
                return Err(UiMessage::new(
                    "error-media-fetch-url",
                    "Missing or empty URL",
                ));
            }
        };
        let headers = match ctx.get_value(PinId(2)) {
            Some(DataValue::String(text)) => parse_headers(&text),
            _ => Vec::new(),
        };
        let request = crate::host::HttpRequest {
            method: "GET".into(),
            url,
            headers,
            body: Vec::new(),
            timeout: crate::host::DEFAULT_HTTP_TIMEOUT,
            multipart: Vec::new(),
            save_to_blob: true,
        };
        ctx.host_state
            .start_http(self.id, request)
            .map(|_| ())
            .map_err(transport_error)
    }

    fn failed(&self, message: UiMessage, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let fallback = message.fallback.clone();
        ctx.host_state
            .error(self.id, message, format!("[ERROR] {fallback}"));
        vec![
            NodeOutput::Data(PinId(12), fallback.into()),
            NodeOutput::Clear(PinId(13)),
            NodeOutput::Execution(PinId(11)),
        ]
    }

    fn media_value(&self, response: crate::host::BlobResponse, ctx: &NodeContext) -> DataValue {
        let mut fields = BTreeMap::new();
        insert_field(&mut fields, "blob", response.blob.clone().into());
        insert_field(&mut fields, "size", (response.size as i64).into());
        if let Some(mime) = response.content_type {
            insert_field(&mut fields, "mime_type", mime.into());
        }
        if let Some(info) = ctx.host_state.blob_store.info(&response.blob)
            && let Some(name) = info.file_name
        {
            insert_field(&mut fields, "file_name", name.into());
        }
        DataValue::Object {
            type_name: Some(format!("media.{}", self.kind)),
            fields,
        }
    }
}

fn valid_kind(value: &DataValue) -> bool {
    matches!(value, DataValue::String(kind) if matches!(kind.as_str(), "photo" | "audio" | "video" | "document"))
}

fn insert_field(fields: &mut BTreeMap<String, ObjectField>, name: &str, value: DataValue) {
    fields.insert(
        name.into(),
        ObjectField {
            data_type: Some(value.get_type()),
            value,
        },
    );
}

fn parse_headers(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (name, value) = line.trim().split_once(':')?;
            Some((name.trim().into(), value.trim().into()))
        })
        .collect()
}

fn transport_error(detail: String) -> UiMessage {
    UiMessage::new(
        "error-media-fetch-transport",
        format!("Media download failed: {detail}"),
    )
    .arg("detail", detail)
}

#[cfg(test)]
mod tests;
