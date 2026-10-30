//! Server-side plugins: discovering `data/plugins/*.wasm`, tracking "enabled / disabled / error" state,
//! and persisting settings in `data/plugin-config/<name>.json` (which may contain secrets:
//! files are written with mode 0600, and secrets are never sent to the editor).
//!
//! A plugin without a config file is treated as enabled with default settings.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use ticknode_core::{PluginChange, TickEngine, effective_settings};
use ticknode_shared::*;
use tracing::{info, warn};

/// Key of the error recorded for a corrupted config file (overwritten by saving settings or enabling).
const CONFIG_CORRUPT_KEY: &str = "error-plugin-config-corrupt";

fn config_corrupt(error: &Option<UiMessage>) -> bool {
    error.as_ref().is_some_and(|e| e.key == CONFIG_CORRUPT_KEY)
}

fn not_found(name: &str) -> UiMessage {
    UiMessage::new("error-plugin-not-found", format!("Plugin {name} not found")).arg("plugin", name)
}

fn not_loaded(name: &str) -> UiMessage {
    UiMessage::new("error-plugin-not-loaded", format!("Plugin {name} is not loaded")).arg("plugin", name)
}

/// Persisted state of a single plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginConfig {
    pub enabled: bool,
    #[serde(default)]
    pub settings: BTreeMap<String, DataValue>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self { enabled: true, settings: BTreeMap::new() }
    }
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
    plugin_data: PathBuf,
    config_dir: PathBuf,
    entries: BTreeMap<String, Entry>,
}

impl Plugins {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            plugins_dir: data_dir.join("plugins"),
            plugin_data: data_dir.join("plugin-data"),
            config_dir: data_dir.join("plugin-config"),
            entries: BTreeMap::new(),
        }
    }

    fn config_path(&self, name: &str) -> PathBuf {
        self.config_dir.join(format!("{name}.json"))
    }

    fn read_config(&self, name: &str) -> anyhow::Result<PluginConfig> {
        let path = self.config_path(name);
        if !path.exists() {
            return Ok(PluginConfig::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("invalid JSON in {}", path.display()))
    }

    fn write_config(&self, name: &str, config: &PluginConfig) -> anyhow::Result<()> {
        write_private(&self.config_dir, &self.config_path(name), &(serde_json::to_string_pretty(config)? + "\n"))
    }

    /// Discovers `*.wasm` files, reads manifests, and enables plugins that are not disabled in config.
    pub fn load_all(&mut self, engine: &mut TickEngine) {
        let Ok(dir) = std::fs::read_dir(&self.plugins_dir) else { return };
        let mut paths: Vec<_> = dir.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths.into_iter().filter(|p| p.extension().is_some_and(|e| e == "wasm")) {
            let file = path.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
            let stem = path.file_stem().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
            let loaded = std::fs::read(&path)
                .map_err(|e| UiMessage::plain(format!("Failed to read the plugin: {e}")))
                .and_then(|bytes| engine.add_plugin(&bytes, Some(&self.plugin_data), |m| m.requested_capabilities.clone()));
            let manifest = match loaded {
                Ok(m) => m,
                Err(e) => {
                    warn!(path = %path.display(), "Failed to load the plugin: {}", e);
                    let key = if self.entries.contains_key(&stem) { file.clone() } else { stem };
                    self.entries.insert(key, Entry { file, config: PluginConfig::default(), error: Some(e), loaded: false, version: String::new(), author: String::new() });
                    continue;
                }
            };
            let (config, mut error) = match self.read_config(&manifest.name) {
                Ok(c) => (c, None),
                Err(e) => {
                    warn!(name = %manifest.name, "Failed to read the plugin config: {:#}", e);
                    (
                        PluginConfig { enabled: false, ..PluginConfig::default() },
                        Some(UiMessage::new(
                            CONFIG_CORRUPT_KEY,
                            format!("The plugin config is corrupted: {e:#}. Saving settings or enabling the plugin will overwrite it"),
                        ).arg("details", format!("{e:#}"))),
                    )
                }
            };
            if config.enabled && error.is_none() {
                let settings = effective_settings(&manifest, &stored(&config));
                match engine.enable_plugin(&manifest.name, &settings) {
                    Ok(_) => info!(name = %manifest.name, version = %manifest.version, capabilities = ?manifest.requested_capabilities, "Plugin loaded"),
                    Err(e) => {
                        warn!(name = %manifest.name, "Failed to enable the plugin: {}", e);
                        error = Some(e);
                    }
                }
                if let Some(e) = engine.plugin_settings_error(&manifest.name) {
                    warn!(name = %manifest.name, "Plugin settings error: {}", e);
                }
            } else if error.is_none() {
                info!(name = %manifest.name, "Plugin is disabled in the config");
            }
            self.entries.insert(manifest.name.clone(), Entry { file, config, error, loaded: true, version: manifest.version, author: manifest.author });
        }
    }

    pub fn infos(&self, engine: &TickEngine) -> Vec<PluginInfo> {
        self.entries.keys().filter_map(|name| self.info(engine, name)).collect()
    }

    pub fn info(&self, engine: &TickEngine, name: &str) -> Option<PluginInfo> {
        let entry = self.entries.get(name)?;
        let manifest = engine.plugin_manifest(name).filter(|_| entry.loaded);
        let settings = manifest.map(|m| m.settings.clone()).unwrap_or_default();
        let effective = manifest.map(|m| effective_settings(m, &stored(&entry.config))).unwrap_or_default();
        // Secrets never leave the server: only whether each secret is set
        let values = settings.iter().map(|def| {
            let value = effective.iter().find(|(k, _)| *k == def.key).map(|(_, v)| v.clone());
            let view = match (def.is_secret(), value) {
                (true, v) => SettingValueView::Secret { is_set: v.is_some() },
                (false, Some(v)) => SettingValueView::Value(v),
                (false, None) => SettingValueView::Unset,
            };
            (def.key.clone(), view)
        }).collect();
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
            capabilities: manifest.map(|m| m.requested_capabilities.clone()).unwrap_or_default(),
            settings,
            values,
        })
    }

    /// Enables or disables a plugin on the fly. Returns `Err` on rejection (dependencies, type conflicts) without changing state.
    pub fn set_enabled(&mut self, engine: &mut TickEngine, name: &str, enabled: bool) -> Result<Outcome, UiMessage> {
        let entry = self.entries.get(name).ok_or_else(|| not_found(name))?;
        if !entry.loaded {
            // The reason the plugin failed to load is the more useful message
            return Err(entry.error.clone().unwrap_or_else(|| not_loaded(name)));
        }
        let manifest = engine.plugin_manifest(name).cloned().ok_or_else(|| not_loaded(name))?;
        let mut config = entry.config.clone();
        config.enabled = enabled;
        let change = match (enabled, engine.plugin_active(name)) {
            (true, false) => {
                let settings = effective_settings(&manifest, &stored(&config));
                match engine.enable_plugin(name, &settings) {
                    Ok(change) => change,
                    Err(e) => {
                        // Keep the error state visible in the panel until enabling succeeds
                        self.entries.get_mut(name).expect("the entry was just found").error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
            (false, true) => engine.disable_plugin(name)?,
            _ => PluginChange::default(),
        };
        let entry = self.entries.get_mut(name).expect("the entry was just found");
        entry.error = None;
        entry.config = config.clone();
        let warning = self.write_config(name, &config)
            .err()
            .map(|e| UiMessage::new(
                "error-plugin-config-not-saved",
                format!("The plugin state was changed, but the config was not saved: {e:#}"),
            ).arg("details", format!("{e:#}")));
        if let Some(w) = &warning {
            warn!("{w}");
        }
        Ok(Outcome { change, warning })
    }

    /// Saves settings and applies them to the plugin (if enabled). A `configure` rejection
    /// is not an error of this action: it is recorded in `PluginInfo::settings_error`.
    pub fn configure(&mut self, engine: &mut TickEngine, name: &str, values: Vec<(String, SettingUpdate)>) -> Result<(), UiMessage> {
        let entry = self.entries.get(name).ok_or_else(|| not_found(name))?;
        let manifest = engine.plugin_manifest(name).filter(|_| entry.loaded).cloned()
            .ok_or_else(|| not_loaded(name))?;
        let mut config = entry.config.clone();
        // Overwrite a corrupted config file when saving new settings
        if config_corrupt(&entry.error) {
            config = PluginConfig { enabled: false, ..PluginConfig::default() };
        }
        for (key, update) in values {
            let def = manifest.settings.iter().find(|d| d.key == key).ok_or_else(|| UiMessage::new("error-setting-unknown", format!("Unknown setting \"{key}\"")).arg("setting", &key))?;
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
        self.write_config(name, &config).map_err(|e| UiMessage::new("error-settings-not-saved", format!("Failed to save the settings: {e:#}")).arg("details", format!("{e:#}")))?;
        let entry = self.entries.get_mut(name).expect("the entry was just found");
        if config_corrupt(&entry.error) {
            entry.error = None;
        }
        entry.config = config.clone();
        if let Err(e) = engine.configure_plugin(name, &effective_settings(&manifest, &stored(&config))) {
            warn!(name, "Plugin settings error: {e}");
        }
        Ok(())
    }
}

fn stored(config: &PluginConfig) -> Vec<(String, DataValue)> {
    config.settings.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Atomic write (tmp + rename) with mode 0600: the file may contain tokens.
fn write_private(dir: &Path, path: &Path, contents: &str) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).with_context(|| format!("failed to write {}", tmp.display()))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path).with_context(|| format!("failed to replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_is_private_and_atomic() {
        let dir = tempfile::tempdir().unwrap();
        let plugins = Plugins::new(dir.path());
        let mut config = PluginConfig::default();
        config.settings.insert("token".into(), DataValue::String("secret".into()));
        plugins.write_config("p", &config).unwrap();
        assert_eq!(plugins.read_config("p").unwrap(), config);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(plugins.config_path("p")).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        // Rewriting leaves no leftover temp files
        plugins.write_config("p", &config).unwrap();
        assert_eq!(std::fs::read_dir(dir.path().join("plugin-config")).unwrap().count(), 1);
        // Missing file defaults to enabled with default settings
        assert_eq!(plugins.read_config("none").unwrap(), PluginConfig::default());
    }
}
