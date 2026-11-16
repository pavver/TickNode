use super::*;
use serde_json::{Value, json};

pub fn keyboard_markup(keyboard: &Keyboard) -> Value {
    let rows: Vec<Vec<Value>> = keyboard
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|b| button_markup(b, keyboard.kind))
                .collect()
        })
        .collect();
    match keyboard.kind {
        KeyboardKind::Inline => json!({ "inline_keyboard": rows }),
        KeyboardKind::Reply => reply_markup(keyboard, rows),
        KeyboardKind::Remove => json!({ "remove_keyboard": true }),
    }
}

fn button_markup(button: &Button, kind: KeyboardKind) -> Value {
    let mut value = json!({"text": button.text});
    match (kind, button.action) {
        (KeyboardKind::Inline, ButtonAction::Url) => {
            value["url"] = button.data.as_deref().unwrap_or("").into();
        }
        (KeyboardKind::Inline, _) => {
            value["callback_data"] = button.data.as_deref().unwrap_or(&button.text).into();
        }
        (_, ButtonAction::Contact) => value["request_contact"] = true.into(),
        (_, ButtonAction::Location) => value["request_location"] = true.into(),
        _ => {}
    }
    value
}

fn reply_markup(keyboard: &Keyboard, rows: Vec<Vec<Value>>) -> Value {
    let mut markup = json!({
        "keyboard": rows,
        "resize_keyboard": keyboard.resize,
        "one_time_keyboard": keyboard.one_time,
        "is_persistent": keyboard.persistent,
    });
    if let Some(placeholder) = keyboard.placeholder.as_deref().map(str::trim)
        && !placeholder.is_empty()
    {
        markup["input_field_placeholder"] = placeholder.into();
    }
    markup
}

pub fn parse_keyboard(value: &DataValue) -> Result<Keyboard, UiError> {
    let kind = match field_string(value, "kind").unwrap_or("inline") {
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
    Ok(Keyboard {
        kind,
        rows: parse_rows(value),
        resize: field_bool(value, "resize").unwrap_or(false),
        one_time: field_bool(value, "one_time").unwrap_or(false),
        placeholder: field_string(value, "placeholder").map(String::from),
        persistent: field_bool(value, "persistent").unwrap_or(false),
    })
}

fn parse_rows(value: &DataValue) -> Vec<Vec<Button>> {
    let Some(DataValue::List { elements, .. }) = field(value, "rows") else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|row| {
            let DataValue::List { elements, .. } = row else {
                return None;
            };
            Some(elements.iter().map(parse_button).collect())
        })
        .collect()
}

fn parse_button(value: &DataValue) -> Button {
    let action = match field_string(value, "action").unwrap_or("callback") {
        "url" => ButtonAction::Url,
        "text" => ButtonAction::Text,
        "contact" => ButtonAction::Contact,
        "location" => ButtonAction::Location,
        _ => ButtonAction::Callback,
    };
    Button {
        text: field_string(value, "text").unwrap_or("").into(),
        action,
        data: field_string(value, "data").map(String::from),
    }
}
