//! Core media struct types.
//!
//! A media value is an immutable reference to an external or platform-specific asset.
//! There is no bytes DataValue; consumers locate media by either `url` or `file_id`.
//!
//! Field layout is identical for all four media structs and is strictly append-only.

use crate::{DataType, DataValue, FieldDefinition, TypeDefinition, UiMessage};

pub const PHOTO: &str = "media.photo";
pub const AUDIO: &str = "media.audio";
pub const VIDEO: &str = "media.video";
pub const DOCUMENT: &str = "media.document";
pub const PREFIX: &str = "media.";

/// Layout of fields common to all media struct types.
///
/// Any future fields must be appended at the end to preserve positional pin IDs in Make/Break nodes.
pub fn media_fields() -> Vec<FieldDefinition> {
    vec![
        FieldDefinition::new("url", "URL", DataType::String)
            .optional()
            .described("HTTP(S) URL of the media asset"),
        FieldDefinition::new("file_id", "File ID", DataType::String)
            .optional()
            .described("Platform-specific media reference identifier"),
        FieldDefinition::new("mime_type", "MIME type", DataType::String)
            .optional()
            .described("MIME content type of the media asset"),
        FieldDefinition::new("file_name", "File name", DataType::String)
            .optional()
            .described("Original file name with extension"),
        FieldDefinition::new("size", "Size", DataType::Number)
            .optional()
            .described("File size in bytes"),
        FieldDefinition::new("width", "Width", DataType::Number)
            .optional()
            .described("Image or video width in pixels"),
        FieldDefinition::new("height", "Height", DataType::Number)
            .optional()
            .described("Image or video height in pixels"),
        FieldDefinition::new("duration", "Duration", DataType::Duration)
            .optional()
            .described("Playback duration for audio or video"),
    ]
}

/// Built-in core media types registered by the engine at construction.
pub fn builtin_media_types() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition::new_struct(PHOTO, "Photo", media_fields())
            .described("Photographic image reference")
            .colored("#26a69a"),
        TypeDefinition::new_struct(AUDIO, "Audio", media_fields())
            .described("Audio recording reference")
            .colored("#ab47bc"),
        TypeDefinition::new_struct(VIDEO, "Video", media_fields())
            .described("Video recording reference")
            .colored("#ef5350"),
        TypeDefinition::new_struct(DOCUMENT, "File", media_fields())
            .described("Document or generic file reference")
            .colored("#78909c"),
    ]
}

/// Source reference of a media object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaSource<'a> {
    FileId(&'a str),
    Url(&'a str),
}

impl<'a> MediaSource<'a> {
    pub fn as_str(&self) -> &'a str {
        match self {
            Self::FileId(s) | Self::Url(s) => s,
        }
    }
}

/// Validates that a media DataValue object has either `file_id` or `url` set and non-empty.
/// Prefers `file_id` when both are present.
pub fn validate_media(value: &DataValue) -> Result<MediaSource<'_>, UiMessage> {
    let DataValue::Object { fields, .. } = value else {
        return Err(UiMessage::new(
            "error-media-missing-source",
            "Media object must have either url or file_id set",
        ));
    };
    let file_id = fields.get("file_id").and_then(|f| match &f.value {
        DataValue::String(s) if !s.trim().is_empty() => Some(s.as_str()),
        _ => None,
    });
    let url = fields.get("url").and_then(|f| match &f.value {
        DataValue::String(s) if !s.trim().is_empty() => Some(s.as_str()),
        _ => None,
    });
    if let Some(id) = file_id {
        Ok(MediaSource::FileId(id))
    } else if let Some(u) = url {
        Ok(MediaSource::Url(u))
    } else {
        Err(UiMessage::new(
            "error-media-missing-source",
            "Media object must have either url or file_id set",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectField;
    use std::collections::BTreeMap;

    #[test]
    fn builtin_media_types_have_identical_fields_and_valid_colors() {
        let types = builtin_media_types();
        assert_eq!(types.len(), 4);
        let fields = media_fields();
        assert_eq!(fields.len(), 8);
        for t in &types {
            assert!(t.name.starts_with(PREFIX));
            assert_eq!(t.fields(), fields.as_slice());
            let color = t.color.as_deref().expect("media type has color");
            assert!(color.starts_with('#') && color.len() == 7);
        }
    }

    #[test]
    fn validate_media_prefers_file_id_over_url() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "file_id".into(),
            ObjectField {
                name: "file_id".into(),
                description: String::new(),
                value: DataValue::String("tg_123".into()),
            },
        );
        fields.insert(
            "url".into(),
            ObjectField {
                name: "url".into(),
                description: String::new(),
                value: DataValue::String("https://example.com/p.jpg".into()),
            },
        );
        let val = DataValue::Object {
            type_name: Some(PHOTO.into()),
            fields,
        };
        assert_eq!(validate_media(&val).unwrap(), MediaSource::FileId("tg_123"));
    }

    #[test]
    fn validate_media_falls_back_to_url_when_file_id_missing() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "url".into(),
            ObjectField {
                name: "url".into(),
                description: String::new(),
                value: DataValue::String("https://example.com/audio.mp3".into()),
            },
        );
        let val = DataValue::Object {
            type_name: Some(AUDIO.into()),
            fields,
        };
        assert_eq!(
            validate_media(&val).unwrap(),
            MediaSource::Url("https://example.com/audio.mp3")
        );
    }

    #[test]
    fn validate_media_rejects_missing_or_empty_sources() {
        let empty_obj = DataValue::Object {
            type_name: Some(VIDEO.into()),
            fields: BTreeMap::new(),
        };
        assert_eq!(
            validate_media(&empty_obj).unwrap_err().key,
            "error-media-missing-source"
        );
        assert_eq!(
            validate_media(&DataValue::Bool(true)).unwrap_err().key,
            "error-media-missing-source"
        );
    }
}
