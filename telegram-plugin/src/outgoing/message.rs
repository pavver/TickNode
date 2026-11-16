use super::*;
use crate::validation;

pub fn parse_outgoing(value: &DataValue) -> Result<Outgoing, UiError> {
    match value {
        DataValue::String(text) => parse_text(text),
        DataValue::Object { type_name, .. } if is_media_type(type_name.as_deref()) => {
            Ok(Outgoing {
                text: None,
                parse_mode: ParseMode::Plain,
                media: Some(parse_media(value)?),
                keyboard: None,
                silent: false,
            })
        }
        DataValue::Object { type_name, .. }
            if type_name.as_deref() == Some("telegram.outgoing") || is_outgoing_object(value) =>
        {
            parse_message(value)
        }
        _ => Err(validation::error(
            "outgoing-value",
            "Expected message text, an outgoing message object, or a media object",
        )),
    }
}

fn parse_text(text: &str) -> Result<Outgoing, UiError> {
    if text.is_empty() {
        return Err(validation::error("text", "Message text is required"));
    }
    validate_text(Some(text), false)?;
    Ok(Outgoing {
        text: Some(text.into()),
        parse_mode: ParseMode::Plain,
        media: None,
        keyboard: None,
        silent: false,
    })
}

fn parse_message(value: &DataValue) -> Result<Outgoing, UiError> {
    let text = field_string(value, "text").map(String::from);
    let media_value = field(value, "media");
    if !text.as_ref().is_some_and(|s| !s.is_empty()) && media_value.is_none() {
        return Err(validation::error(
            "compose-empty",
            "Message requires either text or media",
        ));
    }
    let media = media_value.map(parse_media).transpose()?;
    validate_text(text.as_deref(), media.is_some())?;
    let parse_mode = match field_string(value, "parse_mode") {
        Some("html" | "HTML") => ParseMode::Html,
        Some("markdown_v2" | "MarkdownV2") => ParseMode::MarkdownV2,
        _ => ParseMode::Plain,
    };
    Ok(Outgoing {
        text,
        parse_mode,
        media,
        keyboard: field(value, "keyboard").map(parse_keyboard).transpose()?,
        silent: field_bool(value, "silent").unwrap_or(false),
    })
}

fn validate_text(text: Option<&str>, has_media: bool) -> Result<(), UiError> {
    let text = text.unwrap_or("");
    if has_media {
        if text.chars().count() > 1024 {
            return Err(validation::error(
                "caption",
                "Telegram caption cannot exceed 1024 characters",
            ));
        }
    } else if text.is_empty() || text.chars().count() > 4096 {
        return Err(validation::error(
            "length",
            "Telegram message text must contain 1–4096 characters",
        ));
    }
    Ok(())
}

pub(super) fn parse_media(value: &DataValue) -> Result<OutgoingMedia, UiError> {
    let DataValue::Object {
        type_name: Some(name),
        ..
    } = value
    else {
        return Err(validation::error(
            "media",
            "A media value must have url, file_id, or blob set",
        ));
    };
    if !is_media_type(Some(name)) {
        return Err(validation::error(
            "media",
            "A media value must have url, file_id, or blob set",
        ));
    }
    let text = |key| {
        field_string(value, key)
            .filter(|s| !s.trim().is_empty())
            .map(String::from)
    };
    let source = text("file_id")
        .map(MediaSource::FileId)
        .or_else(|| text("blob").map(MediaSource::Blob))
        .or_else(|| text("url").map(MediaSource::Url))
        .ok_or_else(|| {
            validation::error("media", "A media value must have url, file_id, or blob set")
        })?;
    Ok(OutgoingMedia {
        kind: name.strip_prefix("media.").unwrap_or("photo").into(),
        source,
        file_name: text("file_name"),
        mime_type: text("mime_type"),
    })
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
