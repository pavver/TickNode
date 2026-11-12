//! Server-side plugins: discovering `data/plugins/*.wasm`, tracking "enabled / disabled / error" state,
//! and persisting settings in `data/plugin-config/<name>.json` (which may contain secrets:
//! files are written with mode 0600, and secrets are never sent to the editor).
//!
//! A plugin without a config file is treated as enabled with default settings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ticknode_core::{PluginChange, TickEngine, effective_settings};
use ticknode_shared::*;
use tracing::{info, warn};

pub(crate) mod config;
mod discovery;
pub use config::PluginConfig;
use config::stored;

/// Key of the error recorded for a corrupted config file (overwritten by saving settings or enabling).
const CONFIG_CORRUPT_KEY: &str = "error-plugin-config-corrupt";

fn config_corrupt(error: &Option<UiMessage>) -> bool {
    error.as_ref().is_some_and(|e| e.key == CONFIG_CORRUPT_KEY)
}

fn not_found(name: &str) -> UiMessage {
    UiMessage::new("error-plugin-not-found", format!("Plugin {name} not found")).arg("plugin", name)
}

fn not_loaded(name: &str) -> UiMessage {
    UiMessage::new(
        "error-plugin-not-loaded",
        format!("Plugin {name} is not loaded"),
    )
    .arg("plugin", name)
}

struct Entry {
    file: String,
    config: PluginConfig,
    /// Failed to load, failed to enable, or has a corrupted config file.
    error: Option<UiMessage>,
    /// Component is loaded into the engine (manifest is available).
    loaded: bool,
    /// Fallback metadata for plugins that failed to load (no manifest).
    version: String,
    author: String,
}

/// Result of enabling/disabling a plugin: graph changes and an optional warning for the initiator.
pub struct Outcome {
    pub change: PluginChange,
    pub warning: Option<UiMessage>,
}

pub struct Plugins {
    plugins_dir: PathBuf,
    /// Optional read-only directory with plugins shipped alongside the server (`TICKNODE_BUNDLED_PLUGINS_DIR`).
    bundled_dir: Option<PathBuf>,
    plugin_data: PathBuf,
    config_dir: PathBuf,
    entries: BTreeMap<String, Entry>,
}

impl Plugins {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            plugins_dir: data_dir.join("plugins"),
            bundled_dir: None,
            plugin_data: data_dir.join("plugin-data"),
            config_dir: data_dir.join("plugin-config"),
            entries: BTreeMap::new(),
        }
    }

    /// Scans `dir` for plugins in addition to `<data>/plugins`. A file of the same name in `<data>/plugins` wins.
    pub fn with_bundled_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.bundled_dir = dir;
        self
    }

    pub fn infos(&self, engine: &TickEngine) -> Vec<PluginInfo> {
        self.entries
            .keys()
            .filter_map(|name| self.info(engine, name))
            .collect()
    }

    pub fn info(&self, engine: &TickEngine, name: &str) -> Option<PluginInfo> {
        let entry = self.entries.get(name)?;
        let manifest = engine.plugin_manifest(name).filter(|_| entry.loaded);
        let settings = manifest.map(|m| m.settings.clone()).unwrap_or_default();
        let effective = manifest
            .map(|m| effective_settings(m, &stored(m, &entry.config)))
            .unwrap_or_default();
        // Secrets never leave the server: only whether each secret is set
        let values = settings
            .iter()
            .map(|def| {
                let value = effective
                    .iter()
                    .find(|(k, _)| *k == def.key)
                    .map(|(_, v)| v.clone());
                let view = match (def.is_secret(), value) {
                    (true, v) => SettingValueView::Secret {
                        is_set: v.is_some(),
                    },
                    (false, Some(v)) => SettingValueView::Value(v),
                    (false, None) => SettingValueView::Unset,
                };
                (def.key.clone(), view)
            })
            .collect();
        let state = if entry.error.is_some() {
            PluginState::Error
        } else if engine.plugin_active(name) {
            PluginState::Enabled
        } else {
            PluginState::Disabled
        };
        Some(PluginInfo {
            name: name.to_string(),
            version: manifest.map_or_else(|| entry.version.clone(), |m| m.version.clone()),
            author: manifest.map_or_else(|| entry.author.clone(), |m| m.author.clone()),
            file: entry.file.clone(),
            state,
            error: entry.error.clone(),
            settings_error: engine.plugin_settings_error(name),
            capabilities: manifest
                .map(|m| m.requested_capabilities.clone())
                .unwrap_or_default(),
            settings,
            values,
            resources: manifest.map(|m| m.resources.clone()).unwrap_or_default(),
        })
    }

    /// Enables or disables a plugin on the fly. Returns `Err` on rejection (dependencies, type conflicts) without changing state.
    pub fn set_enabled(
        &mut self,
        engine: &mut TickEngine,
        name: &str,
        enabled: bool,
    ) -> Result<Outcome, UiMessage> {
        let entry = self.entries.get(name).ok_or_else(|| not_found(name))?;
        if !entry.loaded {
            // The reason the plugin failed to load is the more useful message
            return Err(entry.error.clone().unwrap_or_else(|| not_loaded(name)));
        }
        let manifest = engine
            .plugin_manifest(name)
            .cloned()
            .ok_or_else(|| not_loaded(name))?;
        let mut config = entry.config.clone();
        config.enabled = enabled;
        let change = self.change_enabled(engine, &manifest, &config)?;
        let entry = self
            .entries
            .get_mut(name)
            .expect("the entry was just found");
        entry.error = None;
        entry.config = config.clone();
        let warning = self.write_config(name, &config).err().map(|e| {
            UiMessage::new(
                "error-plugin-config-not-saved",
                format!("The plugin state was changed, but the config was not saved: {e:#}"),
            )
            .arg("details", format!("{e:#}"))
        });
        if let Some(w) = &warning {
            warn!("{w}");
        }
        Ok(Outcome { change, warning })
    }

    fn change_enabled(
        &mut self,
        engine: &mut TickEngine,
        manifest: &PluginManifest,
        config: &PluginConfig,
    ) -> Result<PluginChange, UiMessage> {
        let name = &manifest.name;
        let enabled = config.enabled;
        Ok(match (enabled, engine.plugin_active(name)) {
            (true, false) => {
                let settings = effective_settings(&manifest, &stored(&manifest, &config));
                match engine.enable_plugin(name, &settings) {
                    Ok(change) => change,
                    Err(e) => {
                        // Keep the error state visible in the panel until enabling succeeds
                        self.entries
                            .get_mut(name)
                            .expect("the entry was just found")
                            .error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
            (false, true) => engine.disable_plugin(name)?,
            _ => PluginChange::default(),
        })
    }

    /// Saves settings and applies them to the plugin (if enabled). A `configure` rejection
    /// is not an error of this action: it is recorded in `PluginInfo::settings_error`.
    pub fn configure(
        &mut self,
        engine: &mut TickEngine,
        name: &str,
        values: Vec<(String, SettingUpdate)>,
    ) -> Result<(), UiMessage> {
        let entry = self.entries.get(name).ok_or_else(|| not_found(name))?;
        let manifest = engine
            .plugin_manifest(name)
            .filter(|_| entry.loaded)
            .cloned()
            .ok_or_else(|| not_loaded(name))?;
        let mut config = entry.config.clone();
        // Overwrite a corrupted config file when saving new settings
        if config_corrupt(&entry.error) {
            config = PluginConfig {
                enabled: false,
                ..PluginConfig::default()
            };
        }
        apply_setting_updates(name, &manifest, &mut config, values)?;
        self.write_config(name, &config).map_err(|e| {
            UiMessage::new(
                "error-settings-not-saved",
                format!("Failed to save the settings: {e:#}"),
            )
            .arg("details", format!("{e:#}"))
        })?;
        let entry = self
            .entries
            .get_mut(name)
            .expect("the entry was just found");
        if config_corrupt(&entry.error) {
            entry.error = None;
        }
        entry.config = config.clone();
        if let Err(e) = engine.configure_plugin(
            name,
            &effective_settings(&manifest, &stored(&manifest, &config)),
        ) {
            warn!(name, "Plugin settings error: {e}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

fn apply_setting_updates(
    name: &str,
    manifest: &PluginManifest,
    config: &mut PluginConfig,
    values: Vec<(String, SettingUpdate)>,
) -> Result<(), UiMessage> {
    for (key, update) in values {
        let def = manifest
            .settings
            .iter()
            .find(|d| d.key == key)
            .ok_or_else(|| {
                UiMessage::new(
                    "error-setting-unknown",
                    format!("Unknown setting \"{key}\""),
                )
                .arg("setting", &key)
            })?;
        match update {
            SettingUpdate::Set(value) => {
                def.check(name, &value)?;
                config.settings.insert(key, value);
            }
            SettingUpdate::Keep => {}
            SettingUpdate::Clear => {
                config.settings.remove(&key);
            }
        }
    }
    Ok(())
}
