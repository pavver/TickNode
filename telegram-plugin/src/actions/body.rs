//! Pure request body construction. All failures are structured and translatable.
use super::*;
use crate::{
    api::RequestBody,
    outgoing::{self, KeyboardKind, MediaSource},
};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq)]
pub(super) enum RequestParam {
    Text(&'static str, String),
    Bool(&'static str, bool),
    Int(&'static str, i64),
    Json(&'static str, Value),
    Blob {
        field_name: &'static str,
        blob_id: String,
        file_name: String,
        content_type: Option<String>,
    },
}

pub(super) fn render_request_body(params: Vec<RequestParam>) -> RequestBody {
    let has_blob = params
        .iter()
        .any(|p| matches!(p, RequestParam::Blob { .. }));
    if has_blob {
        let mut parts = Vec::with_capacity(params.len());
        for p in params {
            match p {
                RequestParam::Text(name, val) => {
                    parts.push(MultipartPart::text(name, val));
                }
                RequestParam::Bool(name, val) => {
                    parts.push(MultipartPart::text(
                        name,
                        if val { "true" } else { "false" },
                    ));
                }
                RequestParam::Int(name, val) => {
                    parts.push(MultipartPart::text(name, val.to_string()));
                }
                RequestParam::Json(name, val) => {
                    parts.push(MultipartPart::text(name, val.to_string()));
                }
                RequestParam::Blob {
                    field_name,
                    blob_id,
                    file_name,
                    content_type,
                } => {
                    let mut part = MultipartPart::blob(field_name, blob_id).file_name(file_name);
                    if let Some(ct) = content_type {
                        part = part.content_type(ct);
                    }
                    parts.push(part);
                }
            }
        }
        RequestBody::Multipart(parts)
    } else {
        let mut map = serde_json::Map::with_capacity(params.len());
        for p in params {
            match p {
                RequestParam::Text(name, val) => {
                    map.insert(name.into(), Value::String(val));
                }
                RequestParam::Bool(name, val) => {
                    map.insert(name.into(), Value::Bool(val));
                }
                RequestParam::Int(name, val) => {
                    map.insert(name.into(), json!(val));
                }
                RequestParam::Json(name, val) => {
                    map.insert(name.into(), val);
                }
                RequestParam::Blob { .. } => unreachable!("blob handled in multipart branch"),
            }
        }
        RequestBody::Json(Value::Object(map))
    }
}

fn default_file_name(kind: &str, mime: Option<&str>) -> String {
    match kind {
        "photo" => match mime {
            Some("image/png") => "photo.png".into(),
            Some("image/webp") => "photo.webp".into(),
            Some("image/gif") => "photo.gif".into(),
            _ => "photo.jpg".into(),
        },
        "audio" => match mime {
            Some("audio/ogg") => "audio.ogg".into(),
            Some("audio/wav") => "audio.wav".into(),
            Some("audio/m4a" | "audio/mp4") => "audio.m4a".into(),
            _ => "audio.mp3".into(),
        },
        "video" => match mime {
            Some("video/webm") => "video.webm".into(),
            Some("video/quicktime") => "video.mov".into(),
            _ => "video.mp4".into(),
        },
        _ => match mime {
            Some("text/plain") => "file.txt".into(),
            Some("application/pdf") => "file.pdf".into(),
            Some("application/json") => "file.json".into(),
            Some("application/zip") => "file.zip".into(),
            _ => "file.bin".into(),
        },
    }
}

fn id(inputs: &Inputs, pin: u32) -> Option<i64> {
    match inputs.get(PinId(pin)) {
        Some(DataValue::Number(NumberValue::Int(n))) => Some(*n),
        _ => None,
    }
}

pub(super) fn build(
    kind: &str,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
    env: &impl Env,
) -> Result<(&'static str, RequestBody), UiError> {
    let chat = resolve_chat(props, inputs, hub)?;
    match kind {
        "send" => build_send(chat, props, inputs, hub, env),
        "edit" => build_edit(chat, props, inputs).map(|(m, v)| (m, RequestBody::Json(v))),
        _ => build_delete(chat, inputs).map(|(m, v)| (m, RequestBody::Json(v))),
    }
}

fn resolve_chat(props: &Properties, inputs: &Inputs, hub: &Hub) -> Result<i64, UiError> {
    let prop = props.get("chat").map(String::as_str).unwrap_or("");
    let chat = if prop.is_empty() {
        id(inputs, 1)
    } else {
        prop.parse::<i64>().ok()
    }
    .ok_or_else(|| validation::error("chat", "A Telegram chat ID is required"))?;
    if !hub.permits(chat) {
        return Err(UiError::new(
            "telegram-error-forbidden",
            format!("Chat {chat} is not in allowed_chats"),
        )
        .arg("chat", chat.to_string()));
    }
    Ok(chat)
}

fn media_kind_and_method(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "audio" => ("audio", "sendAudio"),
        "video" => ("video", "sendVideo"),
        "document" => ("document", "sendDocument"),
        _ => ("photo", "sendPhoto"),
    }
}

fn attach_thread_and_reply_params(
    params: &mut Vec<RequestParam>,
    chat: i64,
    inputs: &Inputs,
    hub: &Hub,
) {
    let reply = id(inputs, 4);
    if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
        params.push(RequestParam::Int("message_thread_id", topic));
    }
    if let Some(reply) = reply {
        params.push(RequestParam::Json(
            "reply_parameters",
            json!({"message_id": reply}),
        ));
    }
}

fn build_send(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
    env: &impl Env,
) -> Result<(&'static str, RequestBody), UiError> {
    let msg_val = inputs
        .get(PinId(3))
        .ok_or_else(|| validation::error("text", "Message text is required"))?;
    let outgoing = outgoing::parse_outgoing(msg_val)?;

    let mut params = Vec::new();
    params.push(RequestParam::Int("chat_id", chat));

    let method = if let Some(media) = &outgoing.media {
        let (field_name, method) = media_kind_and_method(&media.kind);
        match &media.source {
            MediaSource::FileId(fid) => {
                params.push(RequestParam::Text(field_name, fid.clone()));
            }
            MediaSource::Url(url) => {
                params.push(RequestParam::Text(field_name, url.clone()));
            }
            MediaSource::Blob(blob_id) => {
                let info = env.blob_info(blob_id).ok_or_else(|| {
                    UiError::new(
                        "telegram-error-blob-evicted",
                        "Media blob was evicted or not found in storage",
                    )
                })?;
                let (limit, limit_mb) = if media.kind == "photo" {
                    (10 * 1024 * 1024, 10)
                } else {
                    (50 * 1024 * 1024, 50)
                };
                if info.size > limit {
                    return Err(UiError::new(
                        "telegram-error-upload-size",
                        format!("Media exceeds Telegram upload limit ({limit_mb} MB)"),
                    )
                    .arg("limit", limit_mb.to_string()));
                }
                let content_type = media.mime_type.clone().or(info.content_type);
                let file_name = media
                    .file_name
                    .clone()
                    .or(info.file_name)
                    .unwrap_or_else(|| default_file_name(&media.kind, content_type.as_deref()));
                params.push(RequestParam::Blob {
                    field_name,
                    blob_id: blob_id.clone(),
                    file_name,
                    content_type,
                });
            }
        }
        params.push(RequestParam::Bool("disable_notification", outgoing.silent));
        if let Some(caption) = &outgoing.text
            && !caption.is_empty()
        {
            params.push(RequestParam::Text("caption", caption.clone()));
            if let Some(pm) = outgoing.parse_mode.api_str() {
                params.push(RequestParam::Text("parse_mode", pm.into()));
            }
        }
        if let Some(kb) = &outgoing.keyboard {
            params.push(RequestParam::Json(
                "reply_markup",
                outgoing::keyboard_markup(kb),
            ));
        }
        attach_thread_and_reply_params(&mut params, chat, inputs, hub);
        method
    } else {
        let text = outgoing
            .text
            .ok_or_else(|| validation::error("text", "Message text is required"))?;
        let disable_preview = props.get("disable_preview").map(String::as_str) == Some("yes");
        params.push(RequestParam::Text("text", text));
        params.push(RequestParam::Bool("disable_notification", outgoing.silent));
        params.push(RequestParam::Json(
            "link_preview_options",
            json!({"is_disabled": disable_preview}),
        ));
        if let Some(pm) = outgoing.parse_mode.api_str() {
            params.push(RequestParam::Text("parse_mode", pm.into()));
        }
        if let Some(kb) = &outgoing.keyboard {
            params.push(RequestParam::Json(
                "reply_markup",
                outgoing::keyboard_markup(kb),
            ));
        }
        attach_thread_and_reply_params(&mut params, chat, inputs, hub);
        "sendMessage"
    };

    Ok((method, render_request_body(params)))
}

fn build_edit(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
) -> Result<(&'static str, Value), UiError> {
    let message_id = id(inputs, 2)
        .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?;
    let msg_val = inputs
        .get(PinId(3))
        .ok_or_else(|| validation::error("text", "Message text is required"))?;
    let outgoing = outgoing::parse_outgoing(msg_val)?;

    if outgoing.media.is_some() {
        return Err(validation::error(
            "edit-media",
            "Editing message media is not supported",
        ));
    }

    let text = outgoing
        .text
        .filter(|s| !s.is_empty())
        .ok_or_else(|| validation::error("text", "Message text is required"))?;

    let mut body = json!({
        "chat_id": chat,
        "message_id": message_id,
        "text": text,
    });

    if let Some(pm) = outgoing.parse_mode.api_str() {
        body["parse_mode"] = pm.into();
    }

    if let Some(kb) = &outgoing.keyboard {
        if kb.kind != KeyboardKind::Inline {
            return Err(validation::error(
                "edit-keyboard",
                "Edited messages support only inline keyboards",
            ));
        }
        body["reply_markup"] = outgoing::keyboard_markup(kb);
    } else {
        let keep_buttons = props
            .get("keep_buttons")
            .map(String::as_str)
            .unwrap_or("yes");
        if keep_buttons == "no" {
            body["reply_markup"] = json!({"inline_keyboard": []});
        }
    }

    Ok(("editMessageText", body))
}

fn build_delete(chat: i64, inputs: &Inputs) -> Result<(&'static str, Value), UiError> {
    let message_id = id(inputs, 2)
        .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?;
    Ok((
        "deleteMessage",
        json!({"chat_id": chat, "message_id": message_id}),
    ))
}
