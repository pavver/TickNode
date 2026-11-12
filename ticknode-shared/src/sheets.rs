use crate::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SheetId(pub Uuid);

impl SheetId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn new_v4() -> Self {
        Self(Uuid::new_v4())
    }
}

impl std::fmt::Display for SheetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub const MAX_SHEET_TITLE: usize = 200;
pub const MAX_SHEET_CAPTION: usize = 2000;
pub const MAX_SHEETS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetInfo {
    pub id: SheetId,
    pub title: String,
    pub caption: String,
    pub enabled: bool,
}

impl SheetInfo {
    pub fn new(id: SheetId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            caption: String::new(),
            enabled: true,
        }
    }

    pub fn validated(mut self) -> Self {
        self.title = self.title.trim().to_string();
        if self.title.is_empty() {
            self.title = "Sheet".to_string();
        }
        if let Some((idx, _)) = self.title.char_indices().nth(MAX_SHEET_TITLE) {
            self.title.truncate(idx);
        }
        if let Some((idx, _)) = self.caption.char_indices().nth(MAX_SHEET_CAPTION) {
            self.caption.truncate(idx);
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSummary {
    pub info: SheetInfo,
    pub nodes: u32,
    pub errors: u32,
    pub last_executed_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_info_validated_trims_and_replaces_empty_title() {
        let info = SheetInfo {
            id: SheetId::generate(),
            title: "   ".into(),
            caption: "Some caption".into(),
            enabled: true,
        };
        let valid = info.validated();
        assert_eq!(valid.title, "Sheet");
        assert_eq!(valid.caption, "Some caption");
    }

    #[test]
    fn sheet_info_validated_truncates_overlong_text() {
        let long_title = "a".repeat(300);
        let long_caption = "b".repeat(2500);
        let info = SheetInfo {
            id: SheetId::generate(),
            title: long_title,
            caption: long_caption,
            enabled: false,
        };
        let valid = info.validated();
        assert_eq!(valid.title.chars().count(), MAX_SHEET_TITLE);
        assert_eq!(valid.caption.chars().count(), MAX_SHEET_CAPTION);
        assert!(!valid.enabled);
    }
}
