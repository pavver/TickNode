//! Shared interactive and restored configuration validation.
use std::collections::BTreeMap;
use ticknode_plugin_sdk::*;

#[cfg(test)]
mod tests;

pub type Properties = BTreeMap<String, String>;

pub fn set_text_property(props: &mut Properties, name: &str, value: DataValue) {
    if let DataValue::String(text) = value {
        props.insert(name.into(), text);
    }
}

pub fn set_known_property(props: &mut Properties, name: &str, value: DataValue) {
    if props.contains_key(name) {
        set_text_property(props, name, value);
    }
}
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
    validate_text_property(name, text)?;
    if kind == "keyboard" {
        validate_keyboard_range(name, text)?;
    }
    validate_choice_property(kind, name, text)?;
    if kind == "message" {
        validate_message_regex(name, text, props)?;
    }
    Ok(())
}

fn validate_text_property(name: &str, text: &str) -> Result<(), UiError> {
    match name {
        "chat" | "user" | "topic" => validate_id_property(text),
        "command" if command(text).is_none() => Err(error(
            "command",
            "Command must contain 1–32 lowercase letters, digits or underscores",
        )),
        "placeholder" if text.chars().count() > 64 => Err(error(
            "placeholder",
            "Placeholder text cannot exceed 64 characters",
        )),
        "toast" if text.chars().count() > 200 => Err(error(
            "toast",
            "Callback toast text cannot exceed 200 characters",
        )),
        _ => Ok(()),
    }
}

fn validate_keyboard_range(name: &str, text: &str) -> Result<(), UiError> {
    let (maximum, key, fallback) = match name {
        "count" => (
            24,
            "keyboard-count",
            "Button count must be between 1 and 24",
        ),
        "columns" => (
            8,
            "keyboard-columns",
            "Columns count must be between 1 and 8",
        ),
        _ => return Ok(()),
    };
    if text
        .parse::<u32>()
        .ok()
        .is_some_and(|n| (1..=maximum).contains(&n))
    {
        Ok(())
    } else {
        Err(error(key, fallback))
    }
}

fn validate_id_property(text: &str) -> Result<(), UiError> {
    if !text.is_empty() && text.parse::<i64>().is_err() {
        return Err(error("id", "Enter a signed decimal Telegram ID"));
    }
    Ok(())
}

fn validate_choice_property(kind: &str, name: &str, text: &str) -> Result<(), UiError> {
    if choice_property_is_valid(kind, name, text) {
        Ok(())
    } else {
        Err(error("choice", "Select a valid option"))
    }
}

fn choice_property_is_valid(kind: &str, name: &str, text: &str) -> bool {
    match name {
        "keep_buttons" | "resize" | "one_time" | "persistent" | "include_edited" | "silent"
        | "disable_preview" => ["yes", "no"].contains(&text),
        "action" => ["callback", "url", "text", "contact", "location"].contains(&text),
        "kind" if kind == "download" => {
            ["auto", "photo", "audio", "video", "document"].contains(&text)
        }
        "kind" => ["inline", "reply", "remove"].contains(&text),
        "parse_mode" => {
            ["none", "HTML", "MarkdownV2", "plain", "html", "markdown_v2"].contains(&text)
        }
        "text_mode" => ["any", "equals", "contains", "starts_with", "regex"].contains(&text),
        "data_mode" => ["equals", "prefix"].contains(&text),
        _ => true,
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

pub fn node_properties(definition: NodeDefinition, props: &Properties) -> Vec<(String, DataValue)> {
    definition
        .properties
        .into_iter()
        .map(|property| {
            let value = props.get(&property.name).cloned().unwrap_or_default();
            (property.name, value.into())
        })
        .collect()
}

pub fn node_property(
    kind: &str,
    name: &str,
    value: &DataValue,
    props: &Properties,
) -> Result<(), UiError> {
    let DataValue::String(text) = value else {
        return Err(error("property", "Expected a text property"));
    };
    property(kind, name, text, props)
}
