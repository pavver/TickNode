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
    let prop = |key: &str| props.get(key).map(String::as_str).unwrap_or("");
    let chat = if prop("chat").is_empty() {
        id(inputs, 1)
    } else {
        prop("chat").parse::<i64>().ok()
    }
    .ok_or_else(|| validation::error("chat", "A Telegram chat ID is required"))?;
    if !hub.permits(chat) {
        return Err(UiError::new(
            "telegram-error-forbidden",
            format!("Chat {chat} is not in allowed_chats"),
        )
        .arg("chat", chat.to_string()));
    }
    let mut body = json!({"chat_id":chat});
    if kind != "send" && kind != "send_media" {
        body["message_id"] = id(inputs, 2)
            .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?
            .into();
    }
    if kind == "send" || kind == "edit" {
        let Some(DataValue::String(text)) = inputs.get(PinId(3)) else {
            return Err(validation::error("text", "Message text is required"));
        };
        if text.is_empty() || text.chars().count() > 4096 {
            return Err(validation::error(
                "length",
                "Telegram message text must contain 1–4096 characters",
            ));
        }
        body["text"] = text.clone().into();
        if prop("parse_mode") != "none" {
            body["parse_mode"] = prop("parse_mode").into();
        }
    }
    if kind == "send_media" {
        let media_kind = match prop("kind") {
            "audio" => "audio",
            "video" => "video",
            "document" => "document",
            _ => "photo",
        };
        let Some(media_val) = inputs.get(PinId(3)) else {
            return Err(validation::error(
                "media",
                "A media value must have either url or file_id set",
            ));
        };
        let file_id = field_string(media_val, "file_id").filter(|s| !s.trim().is_empty());
        let url = field_string(media_val, "url").filter(|s| !s.trim().is_empty());
        let media_ref = file_id.or(url).ok_or_else(|| {
            validation::error("media", "A media value must have either url or file_id set")
        })?;
        body[media_kind] = media_ref.into();

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
            if prop("parse_mode") != "none" {
                body["parse_mode"] = prop("parse_mode").into();
            }
        }

        let reply = id(inputs, 5);
        if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
            body["message_thread_id"] = topic.into();
        }
        if let Some(reply) = reply {
            body["reply_parameters"] = json!({"message_id": reply});
        }
        body["disable_notification"] = (prop("silent") == "yes").into();
        attach_keyboard(&mut body, props)?;
    }
    if kind == "send" {
        let reply = id(inputs, 4);
        if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
            body["message_thread_id"] = topic.into();
        }
        if let Some(reply) = reply {
            body["reply_parameters"] = json!({"message_id":reply});
        }
        body["disable_notification"] = (prop("silent") == "yes").into();
        body["link_preview_options"] = json!({"is_disabled":prop("disable_preview") == "yes"});
        attach_keyboard(&mut body, props)?;
    }
    if kind == "edit" {
        let buttons = prop("buttons");
        if !buttons.trim().is_empty() {
            body["reply_markup"] = validation::buttons(buttons)?;
        } else if prop("keep_buttons") == "no" {
            body["reply_markup"] = json!({"inline_keyboard": []});
        }
    }
    Ok((
        match kind {
            "send" => "sendMessage",
            "send_media" => match prop("kind") {
                "audio" => "sendAudio",
                "video" => "sendVideo",
                "document" => "sendDocument",
                _ => "sendPhoto",
            },
            "edit" => "editMessageText",
            _ => "deleteMessage",
        },
        body,
    ))
}
