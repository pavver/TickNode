//! Plugin settings and plugin state for the editor.
//!
//! A plugin declares its settings schema in the manifest (`SettingDefinition`); the server stores values
//! (`data/plugin-config/<name>.json`) and passes them to the plugin via `configure`. Secrets (tokens)
//! are never sent to the editor: `PluginInfo` only reports whether each secret is set or not.

use serde::{Deserialize, Serialize};

use crate::{Capability, DataValue};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SettingKind {
    Text,
    Number,
    Bool,
    /// Token or password: not displayed in the editor and never included in its messages.
    Secret,
    Url,
    /// (value, label) pairs.
    Select(Vec<(String, String)>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingDefinition {
    pub key: String,
    pub title: String,
    pub description: String,
    pub kind: SettingKind,
    /// Without a value (and without a `default`), the plugin is not considered configured.
    pub required: bool,
    pub default: Option<DataValue>,
}

impl SettingDefinition {
    fn new(key: &str, title: &str, kind: SettingKind) -> Self {
        Self { key: key.into(), title: title.into(), description: String::new(), kind, required: false, default: None }
    }

    pub fn text(key: &str, title: &str) -> Self { Self::new(key, title, SettingKind::Text) }
    pub fn number(key: &str, title: &str) -> Self { Self::new(key, title, SettingKind::Number) }
    pub fn bool(key: &str, title: &str) -> Self { Self::new(key, title, SettingKind::Bool) }
    pub fn secret(key: &str, title: &str) -> Self { Self::new(key, title, SettingKind::Secret) }
    pub fn url(key: &str, title: &str) -> Self { Self::new(key, title, SettingKind::Url) }

    /// `options` — (value, label) pairs.
    pub fn select(key: &str, title: &str, options: &[(&str, &str)]) -> Self {
        let options = options.iter().map(|(v, t)| ((*v).into(), (*t).into())).collect();
        Self::new(key, title, SettingKind::Select(options))
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn described(mut self, description: &str) -> Self {
        self.description = description.into();
        self
    }

    pub fn default_value(mut self, value: impl Into<DataValue>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// Validates that the value matches the schema kind.
    pub fn check(&self, value: &DataValue) -> Result<(), String> {
        let ok = match (&self.kind, value) {
            (SettingKind::Text | SettingKind::Secret, DataValue::String(_)) => true,
            (SettingKind::Url, DataValue::String(s)) => s.is_empty() || s.starts_with("http://") || s.starts_with("https://") || s.starts_with("ws://") || s.starts_with("wss://"),
            (SettingKind::Number, DataValue::Number(_)) => true,
            (SettingKind::Bool, DataValue::Bool(_)) => true,
            (SettingKind::Select(options), DataValue::String(s)) => options.iter().any(|(v, _)| v == s),
            _ => false,
        };
        if ok { Ok(()) } else { Err(format!("Налаштування «{}»: недопустиме значення", self.title)) }
    }

    pub fn is_secret(&self) -> bool {
        self.kind == SettingKind::Secret
    }
}

/// What the editor sends for a single setting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettingUpdate {
    Set(DataValue),
    /// Keep unchanged (for secrets: the editor does not know the current value).
    Keep,
    /// Clear the value (falls back to the default if present).
    Clear,
}

/// Setting value representation for the editor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SettingValueView {
    Value(DataValue),
    /// Secret: only whether it is set.
    Secret { is_set: bool },
    /// Neither a saved value nor a default exists.
    Unset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PluginState {
    Enabled,
    Disabled,
    /// Plugin failed to load (see `PluginInfo::error`).
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub author: String,
    /// Filename in the plugins directory.
    pub file: String,
    pub state: PluginState,
    /// Reason for the `Error` state.
    pub error: Option<String>,
    /// Error from the last `configure` call (the plugin remains loaded).
    pub settings_error: Option<String>,
    /// Permissions requested by the plugin (granted by the server).
    pub capabilities: Vec<Capability>,
    pub settings: Vec<SettingDefinition>,
    pub values: Vec<(String, SettingValueView)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_values_are_checked_against_the_schema() {
        let url = SettingDefinition::url("url", "URL");
        assert!(url.check(&DataValue::String("http://homeassistant.local:8123".into())).is_ok());
        assert!(url.check(&DataValue::String("ftp://x".into())).is_err());
        let select = SettingDefinition::select("mode", "Режим", &[("a", "A"), ("b", "B")]);
        assert!(select.check(&DataValue::String("a".into())).is_ok());
        assert!(select.check(&DataValue::String("c".into())).is_err());
        assert!(SettingDefinition::number("n", "N").check(&DataValue::String("1".into())).is_err());
        assert!(SettingDefinition::bool("b", "B").check(&DataValue::Bool(true)).is_ok());
        assert!(SettingDefinition::secret("t", "T").is_secret());
    }

    #[test]
    fn plugin_info_roundtrips_through_postcard() {
        let info = PluginInfo {
            name: "p".into(), version: "1".into(), author: "a".into(), file: "p.wasm".into(),
            state: PluginState::Enabled, error: None, settings_error: Some("e".into()),
            capabilities: vec![Capability::Storage],
            settings: vec![SettingDefinition::secret("token", "Токен").required()],
            values: vec![("token".into(), SettingValueView::Secret { is_set: true })],
        };
        let back: PluginInfo = postcard::from_bytes(&postcard::to_allocvec(&info).unwrap()).unwrap();
        assert_eq!(back, info);
    }
}
