//! Probe node testing blob storage operations: blob_put, blob_info, blob_read.
use ticknode_plugin_sdk::*;

pub struct BlobProbe;

impl PluginNode for BlobProbe {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "test/blob_probe".into(),
            title: "Blob probe".into(),
            category: "Test".into(),
            description: "Exercises host blob store operations".into(),
            help: "Accepts text at Data on In pulse; writes to blob, queries info, reads back, and emits fields.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Data", DataType::String),
                data_out(10, "Blob", DataType::String),
                data_out(11, "Size", DataType::Number),
                data_out(12, "Read", DataType::String),
                exec_out(13, "Done"),
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output> {
        if trigger == Trigger::Pin(PinId(0)) {
            let data = inputs
                .get(PinId(1))
                .and_then(|v| match v {
                    DataValue::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "default-blob-data".into());

            let blob_id =
                match host::blob_put(data.as_bytes(), Some("text/plain"), Some("probe.txt")) {
                    Ok(id) => id,
                    Err(e) => {
                        host::error(&format!("blob_put failed: {e}"));
                        return Vec::new();
                    }
                };

            let info = match host::blob_info(&blob_id) {
                Some(i) => i,
                None => {
                    host::error("blob_info returned None");
                    return Vec::new();
                }
            };

            let read_bytes = match host::blob_read(&blob_id, 0, info.size as u32) {
                Ok(b) => b,
                Err(e) => {
                    host::error(&format!("blob_read failed: {e}"));
                    return Vec::new();
                }
            };
            let read_text = String::from_utf8_lossy(&read_bytes).into_owned();

            return vec![
                Output::Data(PinId(10), blob_id.into()),
                Output::Data(PinId(11), (info.size as i64).into()),
                Output::Data(PinId(12), read_text.into()),
                Output::Exec(PinId(13)),
            ];
        }
        Vec::new()
    }
}
