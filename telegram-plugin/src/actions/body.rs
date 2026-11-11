//! Pure request body construction. All failures are structured and translatable.
use super::*;
use serde_json::json;

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
    if kind != "send" {
        body["message_id"] = id(inputs, 2)
            .ok_or_else(|| validation::error("message", "A Telegram message ID is required"))?
            .into();
    }
    if kind != "delete" {
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
        if !prop("buttons").trim().is_empty() {
            body["reply_markup"] = validation::buttons(prop("buttons"))?;
        }
    }
    Ok((
        match kind {
            "send" => "sendMessage",
            "edit" => "editMessageText",
            _ => "deleteMessage",
        },
        body,
    ))
}
