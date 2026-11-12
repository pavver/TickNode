//! Translatable messages and translation resources shared by the server, plugins and the editor.
//!
//! Logs are never translated (they are always English); only messages shown to the user in the UI
//! travel as a [`UiMessage`]: a Fluent message key with arguments and an English fallback.

use serde::{Deserialize, Serialize};

/// Translation resources: `(locale code, Fluent source)` pairs, e.g. `("uk", "node-delay-title = ...")`.
/// Sent to the editor merged per locale (core first, then every enabled plugin).
pub type Translations = Vec<(String, String)>;

/// A message for the user in the UI: the editor looks `key` up in its translations and substitutes
/// `args`; if the key is empty or unknown, `fallback` (English text, already final) is shown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UiMessage {
    /// Fluent message id (`error-...` for engine errors; plugins use their own prefix). May be empty.
    pub key: String,
    /// Fluent arguments (all strings).
    pub args: Vec<(String, String)>,
    /// English text used when the key cannot be resolved.
    pub fallback: String,
}

impl UiMessage {
    pub fn new(key: impl Into<String>, fallback: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            args: Vec::new(),
            fallback: fallback.into(),
        }
    }

    /// A message without a translation key: only the English text.
    pub fn plain(fallback: impl Into<String>) -> Self {
        Self::new(String::new(), fallback)
    }

    pub fn arg(mut self, name: &str, value: impl ToString) -> Self {
        self.args.push((name.to_string(), value.to_string()));
        self
    }
}

impl std::fmt::Display for UiMessage {
    /// The English fallback text (what goes to logs).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.fallback)
    }
}

impl From<String> for UiMessage {
    fn from(fallback: String) -> Self {
        Self::plain(fallback)
    }
}

impl From<&str> for UiMessage {
    fn from(fallback: &str) -> Self {
        Self::plain(fallback)
    }
}

/// Fluent identifier fragment: lowercase, every character outside `[a-z0-9-]` (including `/`, `.`, `_`)
/// becomes `-`. `math/add` → `math-add`, `ha.state` → `ha-state`, `Control Flow` → `control-flow`.
pub fn fluent_slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_roundtrip() {
        assert_eq!(fluent_slug("math/add"), "math-add");
        assert_eq!(fluent_slug("ha.state_x"), "ha-state-x");
        assert_eq!(fluent_slug("Control Flow"), "control-flow");
        let m = UiMessage::new("k", "Text").arg("a", 1);
        let back: UiMessage = postcard::from_bytes(&postcard::to_allocvec(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        assert_eq!(m.to_string(), "Text");
        assert_eq!(UiMessage::from("x").key, "");
    }
}
