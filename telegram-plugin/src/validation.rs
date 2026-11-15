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
        "chat" | "user" | "topic" => validate_id_property(text)?,
        "command" if command(text).is_none() => {
            return Err(error(
                "command",
                "Command must contain 1–32 lowercase letters, digits or underscores",
            ));
        }
        "buttons" => validate_buttons_property(kind, text, props)?,
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
        "count" if kind == "keyboard" => {
            let valid = text
                .parse::<u32>()
                .ok()
                .is_some_and(|n| (1..=24).contains(&n));
            if !valid {
                return Err(error(
                    "keyboard-count",
                    "Button count must be between 1 and 24",
                ));
            }
        }
        "columns" if kind == "keyboard" => {
            let valid = text
                .parse::<u32>()
                .ok()
                .is_some_and(|n| (1..=8).contains(&n));
            if !valid {
                return Err(error(
                    "keyboard-columns",
                    "Columns count must be between 1 and 8",
                ));
            }
        }
        _ => validate_choice_property(name, text)?,
    }
    if kind == "message" {
        validate_message_regex(name, text, props)?;
    }
    Ok(())
}

fn validate_id_property(text: &str) -> Result<(), UiError> {
    if !text.is_empty() && text.parse::<i64>().is_err() {
        return Err(error("id", "Enter a signed decimal Telegram ID"));
    }
    Ok(())
}

fn validate_buttons_property(kind: &str, text: &str, props: &Properties) -> Result<(), UiError> {
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
    Ok(())
}

fn validate_choice_property(name: &str, text: &str) -> Result<(), UiError> {
    let valid = match name {
        "keyboard" => ["inline", "reply", "remove", "none"].contains(&text),
        "keep_buttons" | "resize" | "one_time" | "persistent" | "include_edited" | "silent"
        | "disable_preview" => ["yes", "no"].contains(&text),
        "action" => ["callback", "url", "text", "contact", "location"].contains(&text),
        "kind" => [
            "photo", "audio", "video", "document", "inline", "reply", "remove",
        ]
        .contains(&text),
        "parse_mode" => {
            ["none", "HTML", "MarkdownV2", "plain", "html", "markdown_v2"].contains(&text)
        }
        "text_mode" => ["any", "equals", "contains", "starts_with", "regex"].contains(&text),
        "data_mode" => ["equals", "prefix"].contains(&text),
        _ => true,
    };
    if !valid {
        Err(error("choice", "Select a valid option"))
    } else {
        Ok(())
    }
}

fn validate_message_regex(name: &str, text: &str, props: &Properties) -> Result<(), UiError> {
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
            row.push(parse_inline_button(button)?);
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

fn parse_inline_button(button: &str) -> Result<Value, UiError> {
    let invalid = || {
        error(
            "buttons",
            "Buttons need Label=callback_data (1–64 bytes) or Label=https://url, separated by |; one row per line",
        )
    };
    let (label, data) = button.trim().split_once('=').ok_or_else(invalid)?;
    let (label, data) = (label.trim(), data.trim());
    if label.is_empty() || data.is_empty() {
        return Err(invalid());
    }
    if data.starts_with("https://") {
        if url::Url::parse(data)
            .ok()
            .is_none_or(|url| url.host_str().is_none())
            || data.chars().any(char::is_whitespace)
        {
            return Err(invalid());
        }
        Ok(json!({"text":label,"url":data}))
    } else {
        if data.contains("://") || data.len() > 64 {
            return Err(invalid());
        }
        Ok(json!({"text":label,"callback_data":data}))
    }
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
            row.push(parse_reply_button(button)?);
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

fn parse_reply_button(button: &str) -> Result<Value, UiError> {
    let invalid = || {
        error(
            "reply-buttons",
            "Reply keyboard requires at least one button: Label, Label=contact or Label=location, separated by |; one row per line",
        )
    };
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
            "contact" => Ok(json!({"text": label, "request_contact": true})),
            "location" => Ok(json!({"text": label, "request_location": true})),
            _ => Err(invalid()),
        }
    } else {
        if button.chars().count() > 128 {
            return Err(invalid());
        }
        Ok(json!({"text": button}))
    }
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

struct ArgParser {
    result: Vec<String>,
    word: String,
    quote: Option<char>,
    escape: bool,
    started: bool,
}

impl ArgParser {
    fn new() -> Self {
        Self {
            result: Vec::new(),
            word: String::new(),
            quote: None,
            escape: false,
            started: false,
        }
    }

    fn step(&mut self, ch: char) {
        if self.escape {
            self.word.push(ch);
            self.escape = false;
            self.started = true;
            return;
        }
        if ch == '\\' {
            self.escape = true;
            self.started = true;
            return;
        }
        if self.quote == Some(ch) {
            self.quote = None;
            return;
        }
        if self.quote.is_none() && matches!(ch, '\'' | '"') {
            self.quote = Some(ch);
            self.started = true;
            return;
        }
        if self.quote.is_none() && ch.is_whitespace() {
            if self.started {
                self.result.push(std::mem::take(&mut self.word));
                self.started = false;
            }
        } else {
            self.word.push(ch);
            self.started = true;
        }
    }

    fn finish(mut self) -> Vec<String> {
        if self.escape {
            self.word.push('\\');
        }
        if self.started {
            self.result.push(self.word);
        }
        self.result
    }
}

/// Shell-like splitting without expansion; quotes group whitespace, backslash escapes a character.
pub fn args(text: &str) -> Vec<String> {
    let mut parser = ArgParser::new();
    for ch in text.chars() {
        parser.step(ch);
    }
    parser.finish()
}
