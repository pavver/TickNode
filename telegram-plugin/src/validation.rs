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
            buttons(text)?;
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
