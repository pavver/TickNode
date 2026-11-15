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

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "kind"
            && let DataValue::String(k) = value
            && matches!(k.as_str(), "photo" | "audio" | "video" | "document")
        {
            self.kind = k;
        }
    }

    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let url = match ctx.get_value(PinId(1)) {
                Some(DataValue::String(u)) if !u.trim().is_empty() => u.trim().to_string(),
                _ => {
                    return vec![
                        NodeOutput::Data(
                            PinId(12),
                            DataValue::String("Missing or empty URL".into()),
                        ),
                        NodeOutput::Execution(PinId(11)),
                    ];
                }
            };

            let mut headers = Vec::new();
            if let Some(DataValue::String(h)) = ctx.get_value(PinId(2)) {
                for line in h.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some((name, val)) = line.split_once(':') {
                        headers.push((name.trim().to_string(), val.trim().to_string()));
                    }
                }
            }

            let request = crate::host::HttpRequest {
                method: "GET".into(),
                url,
                headers,
                body: Vec::new(),
                timeout: crate::host::DEFAULT_HTTP_TIMEOUT,
                multipart: Vec::new(),
                save_to_blob: true,
            };

            match ctx.host_state.start_http(self.id, request) {
                Ok(_) => vec![],
                Err(err) => vec![
                    NodeOutput::Data(PinId(12), DataValue::String(err)),
                    NodeOutput::Execution(PinId(11)),
                ],
            }
        } else {
            vec![]
        }
    }

    fn on_task(
        &mut self,
        _task: TaskId,
        result: TaskResult,
        ctx: &mut NodeContext,
    ) -> Vec<NodeOutput> {
        match result {
            Ok(TaskOutput::Blob(resp)) => {
                let type_name = format!("media.{}", self.kind);
                let mut fields = BTreeMap::new();

                fields.insert(
                    "blob".into(),
                    ObjectField {
                        name: "blob".into(),
                        description: String::new(),
                        value: DataValue::String(resp.blob.clone()),
                    },
                );
                if let Some(ct) = resp.content_type {
                    fields.insert(
                        "mime_type".into(),
                        ObjectField {
                            name: "mime_type".into(),
                            description: String::new(),
                            value: DataValue::String(ct),
                        },
                    );
                }
                fields.insert(
                    "size".into(),
                    ObjectField {
                        name: "size".into(),
                        description: String::new(),
                        value: DataValue::Number(NumberValue::Int(resp.size as i64)),
                    },
                );

                let info = ctx.host_state.blob_store.info(&resp.blob);
                if let Some(info) = info
                    && let Some(file_name) = info.file_name
                {
                    fields.insert(
                        "file_name".into(),
                        ObjectField {
                            name: "file_name".into(),
                            description: String::new(),
                            value: DataValue::String(file_name),
                        },
                    );
                }

                let media_obj = DataValue::Object {
                    type_name: Some(type_name),
                    fields,
                };

                vec![
                    NodeOutput::Data(PinId(13), media_obj),
                    NodeOutput::Execution(PinId(10)),
                ]
            }
            Ok(TaskOutput::Http(resp)) => {
                let err_msg = format!("HTTP {}", resp.status);
                vec![
                    NodeOutput::Data(PinId(12), DataValue::String(err_msg)),
                    NodeOutput::Execution(PinId(11)),
                ]
            }
            Ok(TaskOutput::Delay) => vec![],
            Err(e) => vec![
                NodeOutput::Data(PinId(12), DataValue::String(e)),
                NodeOutput::Execution(PinId(11)),
            ],
        }
    }
}
