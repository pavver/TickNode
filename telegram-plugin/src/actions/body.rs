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
    if params
        .iter()
        .any(|p| matches!(p, RequestParam::Blob { .. }))
    {
        RequestBody::Multipart(params.into_iter().map(RequestParam::into_part).collect())
    } else {
        RequestBody::Json(Value::Object(
            params.into_iter().map(RequestParam::into_json).collect(),
        ))
    }
}

impl RequestParam {
    fn into_part(self) -> MultipartPart {
        match self {
            Self::Blob {
                field_name,
                blob_id,
                file_name,
                content_type,
            } => {
                let mut part = MultipartPart::blob(field_name, blob_id).file_name(file_name);
                if let Some(content_type) = content_type {
                    part = part.content_type(content_type);
                }
                part
            }
            param => {
                let (name, value) = param.into_json();
                let text = match value {
                    Value::String(text) => text,
                    value => value.to_string(),
                };
                MultipartPart::text(name, text)
            }
        }
    }

    fn into_json(self) -> (String, Value) {
        let (name, value) = match self {
            Self::Text(name, value) => (name, Value::String(value)),
            Self::Bool(name, value) => (name, Value::Bool(value)),
            Self::Int(name, value) => (name, json!(value)),
            Self::Json(name, value) => (name, value),
            Self::Blob { .. } => unreachable!("blob handled in multipart branch"),
        };
        (name.into(), value)
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
        params.push(media_param(field_name, media, env)?);
        if let Some(caption) = &outgoing.text
            && !caption.is_empty()
        {
            params.push(RequestParam::Text("caption", caption.clone()));
        }
        method
    } else {
        let text = outgoing
            .text
            .clone()
            .ok_or_else(|| validation::error("text", "Message text is required"))?;
        let disable_preview = props.get("disable_preview").map(String::as_str) == Some("yes");
        params.push(RequestParam::Text("text", text));
        params.push(RequestParam::Json(
            "link_preview_options",
            json!({"is_disabled": disable_preview}),
        ));
        "sendMessage"
    };
    attach_message_params(&mut params, &outgoing);
    attach_thread_and_reply_params(&mut params, chat, inputs, hub);

    Ok((method, render_request_body(params)))
}

fn media_param(
    field_name: &'static str,
    media: &outgoing::OutgoingMedia,
    env: &impl Env,
) -> Result<RequestParam, UiError> {
    let blob_id = match &media.source {
        MediaSource::FileId(value) | MediaSource::Url(value) => {
            return Ok(RequestParam::Text(field_name, value.clone()));
        }
        MediaSource::Blob(id) => id,
    };
    let info = env.blob_info(blob_id).ok_or_else(|| {
        UiError::new(
            "telegram-error-blob-evicted",
            "Media blob was evicted or not found in storage",
        )
    })?;
    let limit_mb = if media.kind == "photo" { 10 } else { 50 };
    if info.size > limit_mb * 1024 * 1024 {
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
    Ok(RequestParam::Blob {
        field_name,
        blob_id: blob_id.clone(),
        file_name,
        content_type,
    })
}

fn attach_message_params(params: &mut Vec<RequestParam>, message: &outgoing::Outgoing) {
    params.push(RequestParam::Bool("disable_notification", message.silent));
    if message.text.as_ref().is_some_and(|s| !s.is_empty())
        && let Some(mode) = message.parse_mode.api_str()
    {
        params.push(RequestParam::Text("parse_mode", mode.into()));
    }
    if let Some(keyboard) = &message.keyboard {
        params.push(RequestParam::Json(
            "reply_markup",
            outgoing::keyboard_markup(keyboard),
        ));
    }
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

    attach_edit_keyboard(&mut body, outgoing.keyboard.as_ref(), props)?;

    Ok(("editMessageText", body))
}

fn attach_edit_keyboard(
    body: &mut Value,
    keyboard: Option<&outgoing::Keyboard>,
    props: &Properties,
) -> Result<(), UiError> {
    if let Some(kb) = keyboard {
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

    Ok(())
}

fn build_delete(chat: i64, inputs: &Inputs) -> Result<(&'static str, Value), UiError> {
    let message_id = id(inputs, 2)
        .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?;
    Ok((
        "deleteMessage",
        json!({"chat_id": chat, "message_id": message_id}),
    ))
}
