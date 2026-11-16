use crate::bindings::ticknode::plugin::{host as wit_host, types as wit};
use crate::*;

/// Logs an info message to the node's console. Logs are always English and never translated.
pub fn log(message: &str) {
    wit_host::log(wit::LogLevel::Info, message);
}

/// Logs a warning (English, like every log).
pub fn warn(message: &str) {
    wit_host::log(wit::LogLevel::Warn, message);
}

/// Logs an error (English, like every log). Use [`UiError`] for errors the user must see in the UI.
pub fn error(message: &str) {
    wit_host::log(wit::LogLevel::Error, message);
}

/// Reports a structured runtime error. Repeat with `log=false` while the failure continues.
/// From plugin-tick, reports an asynchronous settings error instead (cleared by configure).
pub fn node_error(message: &UiError, log: bool) {
    wit_host::node_error(
        &wit::UiError {
            key: message.key.clone(),
            args: message.args.clone(),
            fallback: message.fallback.clone(),
        },
        log,
    );
}

pub fn current_tick() -> u64 {
    wit_host::current_tick()
}

pub fn tick_duration_ms() -> u32 {
    wit_host::tick_duration_ms()
}

pub fn delay_ticks(ticks: u64) -> Result<TaskId, String> {
    wit_host::delay(ticks)
}

/// Delay rounded up to whole ticks.
pub fn delay_ms(ms: u64) -> Result<TaskId, String> {
    let tick = u64::from(tick_duration_ms().max(1));
    delay_ticks(ms.div_ceil(tick))
}

/// Starts a delay in ticks or wall-clock time (rounded up to milliseconds, then ticks).
pub fn delay_for(value: &DataValue) -> Result<TaskId, String> {
    match value {
        DataValue::Ticks(n) => delay_ticks(*n),
        DataValue::Duration(d) => {
            if *d <= ticknode_shared::TimeDelta::zero() {
                delay_ms(0)
            } else {
                let secs = d.num_seconds().max(0) as u128;
                let nanos = d.subsec_nanos().max(0) as u128;
                let total_nanos = secs * 1_000_000_000 + nanos;
                delay_ms(total_nanos.div_ceil(1_000_000).min(u128::from(u64::MAX)) as u64)
            }
        }
        _ => Err("Delay must be a duration in ticks or time".into()),
    }
}

/// Starts HTTP with the manifest's domain permissions. With no current node (`configure`, `tick`),
/// the task belongs to this plugin instance (maximum 8 concurrent) and finishes in [`Plugin::tick`]
/// as [`PluginEvent::HttpDone`]. Cancellation, disable and instance recreation abort the request.
pub fn http(request: HttpRequest) -> Result<TaskId, String> {
    wit_host::start_http(&wit::HttpRequest {
        method: request.method,
        url: request.url,
        headers: request.headers,
        body: request.body,
        timeout_ms: request.timeout_ms,
        multipart: request
            .multipart
            .into_iter()
            .map(|p| wit::MultipartPart {
                name: p.name,
                file_name: p.file_name,
                content_type: p.content_type,
                data: match p.data {
                    PartData::Text(t) => wit::PartData::Text(t),
                    PartData::Blob(b) => wit::PartData::Blob(b),
                },
            })
            .collect(),
        save_to_blob: request.save_to_blob,
    })
}

/// Stores binary data in the host blob store (up to 16 MiB per call); returns the blob ID.
pub fn blob_put(
    data: &[u8],
    content_type: Option<&str>,
    file_name: Option<&str>,
) -> Result<String, String> {
    wit_host::blob_put(data, content_type, file_name)
}

/// Reads a range from a blob in the host store (len clamped to 4 MiB).
pub fn blob_read(id: &str, offset: u64, len: u32) -> Result<Vec<u8>, String> {
    wit_host::blob_read(id, offset, len)
}

/// Retrieves metadata for a blob from the host store.
pub fn blob_info(id: &str) -> Option<BlobInfo> {
    wit_host::blob_info(id).map(|b| BlobInfo {
        id: b.id,
        size: b.size,
        content_type: b.content_type,
        file_name: b.file_name,
        created: b.created,
    })
}

/// Opens a plugin-owned WebSocket (`ws://` or `wss://`; requires [`Capability::WebSocket`] for the host).
/// Returns at once: [`WsEvent::Opened`] (or [`WsEvent::Closed`] if connecting failed) arrives in
/// [`Plugin::tick`]. At most 4 connections per plugin.
pub fn ws_connect(url: &str, headers: &[(String, String)]) -> Result<WsId, String> {
    wit_host::ws_connect(url, headers)
}

/// Sends a text frame. Fails if the connection is unknown or already closed.
pub fn ws_send(id: WsId, text: &str) -> Result<(), String> {
    wit_host::ws_send(id, text)
}

/// Closes a connection; no [`WsEvent::Closed`] is delivered for it afterwards.
pub fn ws_close(id: WsId) {
    wit_host::ws_close(id)
}

/// Publishes the options of a dynamic property (`PropertyDefinition::dynamic(.., source)`), replacing the
/// previous list of that source. Usable from any plugin call. The host drops over-long strings and caps the
/// list at 20 000 options; an empty list removes the source; an unchanged list is not re-sent to editors.
pub fn set_options(source: &str, options: &[PropertyOption]) {
    let options: Vec<wit::PropertyOption> = options
        .iter()
        .map(|o| wit::PropertyOption {
            value: o.value.clone(),
            label: o.label.clone(),
            detail: o.detail.clone(),
            icon: o.icon.clone(),
        })
        .collect();
    wit_host::set_options(source, &options);
}

/// Cancels a task belonging to this node. Returns `false` if the task has already finished.
pub fn cancel(task: TaskId) -> bool {
    wit_host::cancel_task(task)
}

/// Replaces this plugin's read-only value catalog.
pub fn set_value_catalog(entries: &[ticknode_shared::ValueEntry]) {
    use crate::bindings::ticknode::plugin::{host, types as wit};
    let entries: Vec<_> = entries
        .iter()
        .map(|entry| wit::ValueEntry {
            key: entry.key.clone(),
            title: entry.title.clone(),
            icon: entry.icon.clone(),
            fields: entry
                .fields
                .iter()
                .map(|field| wit::ValueField {
                    path: field.path.clone(),
                    title: field.title.clone(),
                    data_type: crate::convert::to_wit_data_type(&field.data_type),
                })
                .collect(),
        })
        .collect();
    host::set_value_catalog(&entries);
}

/// Publishes a watched value, or marks it unavailable.
pub fn set_value(key: &str, value: Option<&DataValue>) {
    let encoded = value.map(crate::convert::encode);
    crate::bindings::ticknode::plugin::host::set_value(key, encoded.as_deref());
}
