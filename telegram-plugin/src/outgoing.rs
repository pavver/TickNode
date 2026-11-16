//! Pure model for outgoing Telegram messages.
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
pub enum MediaSource {
    FileId(String),
    Blob(String),
    Url(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutgoingMedia {
    pub kind: String,
    pub source: MediaSource,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
}

impl OutgoingMedia {
    pub fn reference(&self) -> Option<&str> {
        match &self.source {
            MediaSource::FileId(s) | MediaSource::Url(s) => Some(s.as_str()),
            MediaSource::Blob(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    pub text: Option<String>,
    pub parse_mode: ParseMode,
    pub media: Option<OutgoingMedia>,
    pub keyboard: Option<Keyboard>,
    pub silent: bool,
}

mod keyboard;
mod message;
pub use keyboard::{keyboard_markup, parse_keyboard};
pub use message::parse_outgoing;

#[cfg(test)]
mod tests;
