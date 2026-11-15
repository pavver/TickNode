//! Pure model for outgoing Telegram messages.
use crate::validation;
use serde_json::{Value, json};
use ticknode_plugin_sdk::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    Plain,
    Html,
    MarkdownV2,
}

impl ParseMode {
    pub fn api_str(&self) -> Option<&'static str> {
        match self {
            Self::Plain => None,
            Self::Html => Some("HTML"),
            Self::MarkdownV2 => Some("MarkdownV2"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonAction {
    Callback,
    Url,
    Text,
    Contact,
    Location,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Button {
    pub text: String,
    pub action: ButtonAction,
    pub data: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardKind {
    Inline,
    Reply,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keyboard {
    pub kind: KeyboardKind,
    pub rows: Vec<Vec<Button>>,
    pub resize: bool,
    pub one_time: bool,
    pub placeholder: Option<String>,
    pub persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMedia {
    pub kind: String,
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub text: Option<String>,
    pub parse_mode: ParseMode,
    pub media: Option<OutgoingMedia>,
    pub keyboard: Option<Keyboard>,
    pub silent: bool,
}

pub fn keyboard_markup(keyboard: &Keyboard) -> Value {
    match keyboard.kind {
        KeyboardKind::Inline => {
            let rows: Vec<Value> = keyboard
                .rows
                .iter()
                .map(|row| {
                    let buttons: Vec<Value> = row
                        .iter()
                        .map(|b| match b.action {
                            ButtonAction::Url => {
                                json!({
                                    "text": b.text,
                                    "url": b.data.as_deref().unwrap_or("")
                                })
                            }
                            _ => {
                                json!({
                                    "text": b.text,
                                    "callback_data": b.data.as_deref().unwrap_or(&b.text)
                                })
                            }
                        })
                        .collect();
                    json!(buttons)
                })
                .collect();
            json!({ "inline_keyboard": rows })
        }
        KeyboardKind::Reply => {
            let rows: Vec<Value> = keyboard
                .rows
                .iter()
                .map(|row| {
                    let buttons: Vec<Value> = row
                        .iter()
                        .map(|b| match b.action {
                            ButtonAction::Contact => {
                                json!({
                                    "text": b.text,
                                    "request_contact": true
                                })
                            }
                            ButtonAction::Location => {
                                json!({
                                    "text": b.text,
                                    "request_location": true
                                })
                            }
                            _ => {
                                json!({
                                    "text": b.text
                                })
                            }
                        })
                        .collect();
                    json!(buttons)
                })
                .collect();
            let mut markup = json!({
                "keyboard": rows,
                "resize_keyboard": keyboard.resize,
                "one_time_keyboard": keyboard.one_time,
                "is_persistent": keyboard.persistent,
            });
            if let Some(placeholder) = &keyboard.placeholder {
                let trimmed = placeholder.trim();
                if !trimmed.is_empty() {
                    markup["input_field_placeholder"] = trimmed.into();
                }
            }
            markup
        }
        KeyboardKind::Remove => {
            json!({ "remove_keyboard": true })
        }
    }
}

pub fn parse_keyboard(val: &DataValue) -> Result<Keyboard, UiError> {
    let kind_str = field_string(val, "kind").unwrap_or("inline");
    let kind = match kind_str {
        "reply" => KeyboardKind::Reply,
        "remove" => KeyboardKind::Remove,
        _ => KeyboardKind::Inline,
    };
    if kind == KeyboardKind::Remove {
        return Ok(Keyboard {
            kind,
            rows: Vec::new(),
            resize: false,
            one_time: false,
            placeholder: None,
            persistent: false,
        });
    }

    let mut rows = Vec::new();
    if let Some(DataValue::List { elements, .. }) = field(val, "rows") {
        for row_val in elements {
            if let DataValue::List {
                elements: btn_elements,
                ..
            } = row_val
            {
                let mut row = Vec::new();
                for btn_val in btn_elements {
                    let text = field_string(btn_val, "text").unwrap_or("").to_string();
                    let action = match field_string(btn_val, "action").unwrap_or("callback") {
                        "url" => ButtonAction::Url,
                        "text" => ButtonAction::Text,
                        "contact" => ButtonAction::Contact,
                        "location" => ButtonAction::Location,
                        _ => ButtonAction::Callback,
                    };
                    let data = field_string(btn_val, "data").map(String::from);
                    row.push(Button { text, action, data });
                }
                rows.push(row);
            }
        }
    }

    let resize = field_bool(val, "resize").unwrap_or(false);
    let one_time = field_bool(val, "one_time").unwrap_or(false);
    let placeholder = field_string(val, "placeholder").map(String::from);
    let persistent = field_bool(val, "persistent").unwrap_or(false);

    Ok(Keyboard {
        kind,
        rows,
        resize,
        one_time,
        placeholder,
        persistent,
    })
}

pub fn parse_outgoing(value: &DataValue) -> Result<Outgoing, UiError> {
    match value {
        DataValue::String(text) => {
            if text.is_empty() {
                return Err(validation::error("text", "Message text is required"));
            }
            if text.chars().count() > 4096 {
                return Err(validation::error(
                    "length",
                    "Telegram message text must contain 1–4096 characters",
                ));
            }
            Ok(Outgoing {
                text: Some(text.clone()),
                parse_mode: ParseMode::Plain,
                media: None,
                keyboard: None,
                silent: false,
            })
        }
        DataValue::Object { type_name, .. } if is_media_type(type_name.as_deref()) => {
            let kind = type_name
                .as_deref()
                .and_then(|n| n.strip_prefix("media."))
                .unwrap_or("photo")
                .to_string();
            let file_id = field_string(value, "file_id").filter(|s| !s.trim().is_empty());
            let url = field_string(value, "url").filter(|s| !s.trim().is_empty());
            let reference = file_id.or(url).ok_or_else(|| {
                validation::error("media", "A media value must have either url or file_id set")
            })?;
            Ok(Outgoing {
                text: None,
                parse_mode: ParseMode::Plain,
                media: Some(OutgoingMedia {
                    kind,
                    reference: reference.to_string(),
                }),
                keyboard: None,
                silent: false,
            })
        }
        DataValue::Object { type_name, .. }
            if type_name.as_deref() == Some("telegram.outgoing") || is_outgoing_object(value) =>
        {
            let text = field_string(value, "text").map(String::from);
            let media_val = field(value, "media");

            let has_text = text.as_ref().is_some_and(|s| !s.is_empty());
            let has_media = media_val.is_some();

            if !has_text && !has_media {
                return Err(validation::error(
                    "compose-empty",
                    "Message requires either text or media",
                ));
            }

            let media = if let Some(m) = media_val {
                let kind = match m {
                    DataValue::Object {
                        type_name: Some(name),
                        ..
                    } if name.starts_with("media.") => {
                        name.strip_prefix("media.").unwrap_or("photo").to_string()
                    }
                    _ => "photo".to_string(),
                };
                let file_id = field_string(m, "file_id").filter(|s| !s.trim().is_empty());
                let url = field_string(m, "url").filter(|s| !s.trim().is_empty());
                let reference = file_id.or(url).ok_or_else(|| {
                    validation::error("media", "A media value must have either url or file_id set")
                })?;
                Some(OutgoingMedia {
                    kind,
                    reference: reference.to_string(),
                })
            } else {
                None
            };

            if media.is_some() {
                if let Some(t) = &text
                    && t.chars().count() > 1024
                {
                    return Err(validation::error(
                        "caption",
                        "Telegram caption cannot exceed 1024 characters",
                    ));
                }
            } else {
                let Some(t) = &text else {
                    return Err(validation::error("text", "Message text is required"));
                };
                if t.is_empty() || t.chars().count() > 4096 {
                    return Err(validation::error(
                        "length",
                        "Telegram message text must contain 1–4096 characters",
                    ));
                }
            }

            let parse_mode = match field_string(value, "parse_mode") {
                Some("html") | Some("HTML") => ParseMode::Html,
                Some("markdown_v2") | Some("MarkdownV2") => ParseMode::MarkdownV2,
                _ => ParseMode::Plain,
            };

            let silent = field_bool(value, "silent").unwrap_or(false);
            let keyboard = if let Some(kb) = field(value, "keyboard") {
                Some(parse_keyboard(kb)?)
            } else {
                None
            };

            Ok(Outgoing {
                text,
                parse_mode,
                media,
                keyboard,
                silent,
            })
        }
        _ => Err(validation::error(
            "outgoing-value",
            "Expected message text, an outgoing message object, or a media object",
        )),
    }
}

fn is_media_type(name: Option<&str>) -> bool {
    matches!(
        name,
        Some("media.photo" | "media.audio" | "media.video" | "media.document")
    )
}

fn is_outgoing_object(value: &DataValue) -> bool {
    field(value, "text").is_some()
        || field(value, "media").is_some()
        || field(value, "keyboard").is_some()
        || field(value, "parse_mode").is_some()
        || field(value, "silent").is_some()
}

#[cfg(test)]
mod tests;
