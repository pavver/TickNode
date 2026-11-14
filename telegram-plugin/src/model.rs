//! Telegram wire models and typed graph values. Unknown API fields are ignored.
use serde::{Deserialize, Serialize};
use ticknode_plugin_sdk::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct User {
    pub id: i64,
    #[serde(default)]
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
    #[serde(rename = "language_code")]
    pub language: Option<String>,
    #[serde(default)]
    pub is_bot: bool,
}
impl User {
    pub fn value(&self) -> DataValue {
        StructValue::new("telegram.user")
            .field("id", self.id)
            .field("first_name", self.first_name.clone())
            .field_opt("last_name", self.last_name.clone())
            .field_opt("username", self.username.clone())
            .field_opt("language", self.language.clone())
            .field("is_bot", self.is_bot)
            .build()
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Chat {
    pub id: i64,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub title: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Topic {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PhotoSize {
    pub file_id: String,
    pub width: u32,
    pub height: u32,
    pub file_size: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Audio {
    pub file_id: String,
    pub duration: i64,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Video {
    pub file_id: String,
    pub width: u32,
    pub height: u32,
    pub duration: i64,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Document {
    pub file_id: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub message_id: i64,
    pub chat: Chat,
    pub message_thread_id: Option<i64>,
    pub from: Option<User>,
    pub text: Option<String>,
    pub caption: Option<String>,
    #[serde(default)]
    pub date: i64,
    pub reply_to_message: Option<Box<Message>>,
    pub forum_topic_created: Option<Topic>,
    pub forum_topic_edited: Option<Topic>,
    pub photo: Option<Vec<PhotoSize>>,
    pub audio: Option<Audio>,
    pub video: Option<Video>,
    pub document: Option<Document>,
}
impl Message {
    pub fn value(&self) -> DataValue {
        let date = chrono::DateTime::from_timestamp(self.date, 0).unwrap_or_default();
        let text = self.text.clone().or_else(|| self.caption.clone());
        let photo_val = self.photo.as_ref().and_then(|photos| {
            let largest = photos
                .iter()
                .max_by_key(|p| p.width as u64 * p.height as u64)?;
            Some(
                StructValue::new("media.photo")
                    .field("file_id", largest.file_id.clone())
                    .field("width", largest.width as i64)
                    .field("height", largest.height as i64)
                    .field_opt("size", largest.file_size)
                    .build(),
            )
        });
        let audio_val = self.audio.as_ref().map(|a| {
            StructValue::new("media.audio")
                .field("file_id", a.file_id.clone())
                .field("duration", chrono::TimeDelta::seconds(a.duration))
                .field_opt("file_name", a.file_name.clone())
                .field_opt("mime_type", a.mime_type.clone())
                .field_opt("size", a.file_size)
                .build()
        });
        let video_val = self.video.as_ref().map(|v| {
            StructValue::new("media.video")
                .field("file_id", v.file_id.clone())
                .field("width", v.width as i64)
                .field("height", v.height as i64)
                .field("duration", chrono::TimeDelta::seconds(v.duration))
                .field_opt("file_name", v.file_name.clone())
                .field_opt("mime_type", v.mime_type.clone())
                .field_opt("size", v.file_size)
                .build()
        });
        let doc_val = self.document.as_ref().map(|d| {
            StructValue::new("media.document")
                .field("file_id", d.file_id.clone())
                .field_opt("file_name", d.file_name.clone())
                .field_opt("mime_type", d.mime_type.clone())
                .field_opt("size", d.file_size)
                .build()
        });
        StructValue::new("telegram.message")
            .field("chat", self.chat.id)
            .field_opt("topic", self.message_thread_id)
            .field("id", self.message_id)
            .field_opt("from", self.from.as_ref().map(User::value))
            .field_opt("text", text)
            .field("date", DataValue::DateTime(date))
            .field_opt(
                "reply_to",
                self.reply_to_message.as_ref().map(|m| m.message_id),
            )
            .field_opt("chat_title", self.chat.title.clone())
            .field("chat_type", self.chat.kind.clone())
            .field_opt("photo", photo_val)
            .field_opt("audio", audio_val)
            .field_opt("video", video_val)
            .field_opt("document", doc_val)
            .build()
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Callback {
    pub id: String,
    pub from: User,
    pub message: Option<Message>,
    pub data: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct MemberUpdate {
    pub chat: Chat,
    pub from: User,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
    pub edited_message: Option<Message>,
    pub callback_query: Option<Callback>,
    pub my_chat_member: Option<MemberUpdate>,
}
impl Update {
    pub fn message(&self) -> Option<&Message> {
        self.message
            .as_ref()
            .or(self.edited_message.as_ref())
            .or_else(|| self.callback_query.as_ref()?.message.as_ref())
    }
    pub fn chat(&self) -> Option<i64> {
        self.message()
            .map(|m| m.chat.id)
            .or_else(|| Some(self.my_chat_member.as_ref()?.chat.id))
    }
}
