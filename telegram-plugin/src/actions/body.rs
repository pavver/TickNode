//! Pure request body construction. All failures are structured and translatable.
use super::*;
use serde_json::json;

fn id(inputs: &Inputs, pin: u32) -> Option<i64> {
    match inputs.get(PinId(pin)) {
        Some(DataValue::Number(NumberValue::Int(n))) => Some(*n),
        _ => None,
    }
}
fn attach_keyboard(body: &mut Value, props: &Properties) -> Result<(), UiError> {
    let keyboard = props
        .get("keyboard")
        .map(String::as_str)
        .unwrap_or("inline");
    match keyboard {
        "inline" => {
            let buttons = props.get("buttons").map(String::as_str).unwrap_or("");
            if !buttons.trim().is_empty() {
                body["reply_markup"] = validation::buttons(buttons)?;
            }
        }
        "reply" => {
            let buttons = props.get("buttons").map(String::as_str).unwrap_or("");
            body["reply_markup"] = validation::reply_keyboard(buttons, props)?;
        }
        "remove" => {
            body["reply_markup"] = json!({"remove_keyboard": true});
        }
        "none" => {}
        _ => return Err(validation::error("choice", "Select a valid option")),
    }
    Ok(())
}
pub(super) fn build(
    kind: &str,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
) -> Result<(&'static str, Value), UiError> {
    let chat = resolve_chat(props, inputs, hub)?;
    match kind {
        "send" => build_send(chat, props, inputs, hub),
        "send_media" => build_send_media(chat, props, inputs, hub),
        "edit" => build_edit(chat, props, inputs),
        _ => build_delete(chat, inputs),
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

fn build_send(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
) -> Result<(&'static str, Value), UiError> {
    let prop = |key: &str| props.get(key).map(String::as_str).unwrap_or("");
    let Some(DataValue::String(text)) = inputs.get(PinId(3)) else {
        return Err(validation::error("text", "Message text is required"));
    };
    if text.is_empty() || text.chars().count() > 4096 {
        return Err(validation::error(
            "length",
            "Telegram message text must contain 1–4096 characters",
        ));
    }
    let mut body = json!({
        "chat_id": chat,
        "text": text,
        "disable_notification": prop("silent") == "yes",
        "link_preview_options": {"is_disabled": prop("disable_preview") == "yes"},
    });
    if prop("parse_mode") != "none" {
        body["parse_mode"] = prop("parse_mode").into();
    }

    let reply = id(inputs, 4);
    if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
        body["message_thread_id"] = topic.into();
    }
    if let Some(reply) = reply {
        body["reply_parameters"] = json!({"message_id": reply});
    }
    attach_keyboard(&mut body, props)?;
    Ok(("sendMessage", body))
}

fn extract_media_ref(inputs: &Inputs) -> Result<&str, UiError> {
    let Some(media_val) = inputs.get(PinId(3)) else {
        return Err(validation::error(
            "media",
            "A media value must have either url or file_id set",
        ));
    };
    let file_id = field_string(media_val, "file_id").filter(|s| !s.trim().is_empty());
    let url = field_string(media_val, "url").filter(|s| !s.trim().is_empty());
    file_id.or(url).ok_or_else(|| {
        validation::error("media", "A media value must have either url or file_id set")
    })
}

fn attach_caption(body: &mut Value, inputs: &Inputs, parse_mode: &str) -> Result<(), UiError> {
    if let Some(DataValue::String(caption)) = inputs.get(PinId(4))
        && !caption.is_empty()
    {
        if caption.chars().count() > 1024 {
            return Err(validation::error(
                "caption",
                "Telegram caption cannot exceed 1024 characters",
            ));
        }
        body["caption"] = caption.clone().into();
        if parse_mode != "none" {
            body["parse_mode"] = parse_mode.into();
        }
    }
    Ok(())
}

fn media_kind_and_method(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "audio" => ("audio", "sendAudio"),
        "video" => ("video", "sendVideo"),
        "document" => ("document", "sendDocument"),
        _ => ("photo", "sendPhoto"),
    }
}

fn attach_thread_and_reply(body: &mut Value, chat: i64, inputs: &Inputs, hub: &Hub) {
    let reply = id(inputs, 5);
    if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
        body["message_thread_id"] = topic.into();
    }
    if let Some(reply) = reply {
        body["reply_parameters"] = json!({"message_id": reply});
    }
}

fn build_send_media(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
) -> Result<(&'static str, Value), UiError> {
    let prop = |key: &str| props.get(key).map(String::as_str).unwrap_or("");
    let (media_kind, method) = media_kind_and_method(prop("kind"));
    let media_ref = extract_media_ref(inputs)?;

    let mut body = json!({
        "chat_id": chat,
        media_kind: media_ref,
        "disable_notification": prop("silent") == "yes",
    });

    attach_caption(&mut body, inputs, prop("parse_mode"))?;
    attach_thread_and_reply(&mut body, chat, inputs, hub);
    attach_keyboard(&mut body, props)?;

    Ok((method, body))
}

fn build_edit(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
) -> Result<(&'static str, Value), UiError> {
    let prop = |key: &str| props.get(key).map(String::as_str).unwrap_or("");
    let message_id = id(inputs, 2)
        .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?;
    let Some(DataValue::String(text)) = inputs.get(PinId(3)) else {
        return Err(validation::error("text", "Message text is required"));
    };
    if text.is_empty() || text.chars().count() > 4096 {
        return Err(validation::error(
            "length",
            "Telegram message text must contain 1–4096 characters",
        ));
    }
    let mut body = json!({
        "chat_id": chat,
        "message_id": message_id,
        "text": text,
    });
    if prop("parse_mode") != "none" {
        body["parse_mode"] = prop("parse_mode").into();
    }
    let buttons = prop("buttons");
    if !buttons.trim().is_empty() {
        body["reply_markup"] = validation::buttons(buttons)?;
    } else if prop("keep_buttons") == "no" {
        body["reply_markup"] = json!({"inline_keyboard": []});
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
