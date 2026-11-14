//! Shared interactive and restored configuration validation.
use serde_json::{Value, json};
use std::collections::BTreeMap;
use ticknode_plugin_sdk::*;

pub type Properties = BTreeMap<String, String>;
pub fn error(key: &str, text: &str) -> UiError {
    UiError::new(format!("telegram-error-{key}"), text)
}
pub fn command(text: &str) -> Option<&str> {
    let name = text.strip_prefix('/').unwrap_or(text);
    (!name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'))
    .then_some(name)
}
pub fn property(kind: &str, name: &str, text: &str, props: &Properties) -> Result<(), UiError> {
    match name {
        "chat" | "user" | "topic" if !text.is_empty() && text.parse::<i64>().is_err() => {
            return Err(error("id", "Enter a signed decimal Telegram ID"));
        }
        "command" if command(text).is_none() => {
            return Err(error(
                "command",
                "Command must contain 1–32 lowercase letters, digits or underscores",
            ));
        }
        "buttons" => {
            if kind == "edit" {
                buttons(text)?;
            } else {
                let keyboard = props
                    .get("keyboard")
                    .map(String::as_str)
                    .unwrap_or("inline");
                if keyboard == "reply" {
                    reply_buttons(text)?;
                } else if keyboard == "inline" {
                    buttons(text)?;
                }
            }
        }
        "keyboard" if !["inline", "reply", "remove", "none"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "keep_buttons" | "resize" | "one_time" | "persistent" if !["yes", "no"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "placeholder" if text.chars().count() > 64 => {
            return Err(error(
                "placeholder",
                "Placeholder text cannot exceed 64 characters",
            ));
        }
        "toast" if text.chars().count() > 200 => {
            return Err(error(
                "toast",
                "Callback toast text cannot exceed 200 characters",
            ));
        }
        "include_edited" | "silent" | "disable_preview" if !["yes", "no"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "kind" if !["photo", "audio", "video", "document"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "parse_mode" if !["none", "HTML", "MarkdownV2"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "text_mode" if !["any", "equals", "contains", "starts_with", "regex"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        "data_mode" if !["equals", "prefix"].contains(&text) => {
            return Err(error("choice", "Select a valid option"));
        }
        _ => {}
    }
    if kind == "message" {
        let mode = if name == "text_mode" {
            text
        } else {
            props.get("text_mode").map(String::as_str).unwrap_or("any")
        };
        let pattern = if name == "pattern" {
            text
        } else {
            props.get("pattern").map(String::as_str).unwrap_or("")
        };
        if mode == "regex"
            && regex::RegexBuilder::new(pattern)
                .size_limit(256_000)
                .build()
                .is_err()
        {
            return Err(error(
                "regex",
                "Enter a valid regular expression (maximum compiled size 256 KB)",
            ));
        }
    }
    Ok(())
}
pub fn stored(kind: &str, props: &Properties) -> Result<(), UiError> {
    for (name, value) in props {
        property(kind, name, value, props)?;
    }
    Ok(())
}
pub fn buttons(text: &str) -> Result<Value, UiError> {
    let invalid = || {
        error(
            "buttons",
            "Buttons need Label=callback_data (1–64 bytes) or Label=https://url, separated by |; one row per line",
        )
    };
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut row = Vec::new();
        for button in line.split('|') {
            let (label, data) = button.trim().split_once('=').ok_or_else(invalid)?;
            let (label, data) = (label.trim(), data.trim());
            if label.is_empty() || data.is_empty() {
                return Err(invalid());
            }
            row.push(if data.starts_with("https://") {
                if url::Url::parse(data)
                    .ok()
                    .is_none_or(|url| url.host_str().is_none())
                    || data.chars().any(char::is_whitespace)
                {
                    return Err(invalid());
                }
                json!({"text":label,"url":data})
            } else {
                if data.contains("://") || data.len() > 64 {
                    return Err(invalid());
                }
                json!({"text":label,"callback_data":data})
            });
        }
        if row.len() > 8 {
            return Err(invalid());
        }
        rows.push(row);
    }
    if rows.iter().map(Vec::len).sum::<usize>() > 100 {
        return Err(invalid());
    }
    Ok(json!({"inline_keyboard":rows}))
}

pub fn reply_buttons(text: &str) -> Result<Vec<Vec<Value>>, UiError> {
    let invalid = || {
        error(
            "reply-buttons",
            "Reply keyboard requires at least one button: Label, Label=contact or Label=location, separated by |; one row per line",
        )
    };
    let mut rows = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let mut row = Vec::new();
        for button in line.split('|') {
            let button = button.trim();
            if button.is_empty() {
                return Err(invalid());
            }
            if let Some((label, action)) = button.split_once('=') {
                let (label, action) = (label.trim(), action.trim());
                if label.is_empty() || label.chars().count() > 128 {
                    return Err(invalid());
                }
                match action {
                    "contact" => row.push(json!({"text": label, "request_contact": true})),
                    "location" => row.push(json!({"text": label, "request_location": true})),
                    _ => return Err(invalid()),
                }
            } else {
                if button.chars().count() > 128 {
                    return Err(invalid());
                }
                row.push(json!({"text": button}));
            }
        }
        if row.len() > 12 {
            return Err(invalid());
        }
        rows.push(row);
    }
    let total: usize = rows.iter().map(Vec::len).sum();
    if rows.is_empty() || total > 100 {
        return Err(invalid());
    }
    Ok(rows)
}

pub fn reply_keyboard(text: &str, props: &Properties) -> Result<Value, UiError> {
    let rows = reply_buttons(text)?;
    let mut markup = json!({
        "keyboard": rows,
        "resize_keyboard": props.get("resize").map(String::as_str).unwrap_or("yes") == "yes",
        "one_time_keyboard": props.get("one_time").map(String::as_str).unwrap_or("no") == "yes",
        "is_persistent": props.get("persistent").map(String::as_str).unwrap_or("no") == "yes",
    });
    let placeholder = props
        .get("placeholder")
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    if !placeholder.is_empty() {
        markup["input_field_placeholder"] = placeholder.into();
    }
    Ok(markup)
}

/// Shell-like splitting without expansion; quotes group whitespace, backslash escapes a character.
pub fn args(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let (mut word, mut quote, mut escape, mut started) = (String::new(), None, false, false);
    for ch in text.chars() {
        if escape {
            word.push(ch);
            escape = false;
            started = true;
            continue;
        }
        if ch == '\\' {
            escape = true;
            started = true;
            continue;
        }
        if quote == Some(ch) {
            quote = None;
            continue;
        }
        if quote.is_none() && matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
            continue;
        }
        if quote.is_none() && ch.is_whitespace() {
            if started {
                result.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if escape {
        word.push('\\');
    }
    if started {
        result.push(word);
    }
    result
}
