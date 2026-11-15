//! Pure request body construction. All failures are structured and translatable.
use super::*;
use crate::outgoing::{self, KeyboardKind};
use serde_json::{Value, json};

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
    let chat = resolve_chat(props, inputs, hub)?;
    match kind {
        "send" => build_send(chat, props, inputs, hub),
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

fn media_kind_and_method(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "audio" => ("audio", "sendAudio"),
        "video" => ("video", "sendVideo"),
        "document" => ("document", "sendDocument"),
        _ => ("photo", "sendPhoto"),
    }
}

fn attach_thread_and_reply(body: &mut Value, chat: i64, inputs: &Inputs, hub: &Hub) {
    let reply = id(inputs, 4);
    if let Some(topic) = id(inputs, 2).or_else(|| hub.reply_topic(chat, reply?)) {
        body["message_thread_id"] = topic.into();
    }
    if let Some(reply) = reply {
        body["reply_parameters"] = json!({"message_id": reply});
    }
}

fn build_send(
    chat: i64,
    props: &Properties,
    inputs: &Inputs,
    hub: &Hub,
) -> Result<(&'static str, Value), UiError> {
    let msg_val = inputs
        .get(PinId(3))
        .ok_or_else(|| validation::error("text", "Message text is required"))?;
    let outgoing = outgoing::parse_outgoing(msg_val)?;

    if let Some(media) = outgoing.media {
        let (field_name, method) = media_kind_and_method(&media.kind);
        let mut body = json!({
            "chat_id": chat,
            field_name: media.reference,
            "disable_notification": outgoing.silent,
        });
        if let Some(caption) = &outgoing.text
            && !caption.is_empty()
        {
            body["caption"] = caption.clone().into();
            if let Some(pm) = outgoing.parse_mode.api_str() {
                body["parse_mode"] = pm.into();
            }
        }
        if let Some(kb) = &outgoing.keyboard {
            body["reply_markup"] = outgoing::keyboard_markup(kb);
        }
        attach_thread_and_reply(&mut body, chat, inputs, hub);
        Ok((method, body))
    } else {
        let text = outgoing
            .text
            .ok_or_else(|| validation::error("text", "Message text is required"))?;
        let disable_preview = props.get("disable_preview").map(String::as_str) == Some("yes");
        let mut body = json!({
            "chat_id": chat,
            "text": text,
            "disable_notification": outgoing.silent,
            "link_preview_options": {"is_disabled": disable_preview},
        });
        if let Some(pm) = outgoing.parse_mode.api_str() {
            body["parse_mode"] = pm.into();
        }
        if let Some(kb) = &outgoing.keyboard {
            body["reply_markup"] = outgoing::keyboard_markup(kb);
        }
        attach_thread_and_reply(&mut body, chat, inputs, hub);
        Ok(("sendMessage", body))
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
