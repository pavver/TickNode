use super::*;
use anyhow::Context;
use serde::{Deserialize, Serialize};

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

impl Plugins {
    pub(super) fn config_path(&self, name: &str) -> PathBuf {
        self.config_dir.join(format!("{name}.json"))
    }

    pub(super) fn read_config(&self, name: &str) -> anyhow::Result<PluginConfig> {
        let path = self.config_path(name);
        if !path.exists() {
            return Ok(PluginConfig::default());
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("invalid JSON in {}", path.display()))
    }

    pub(super) fn write_config(&self, name: &str, config: &PluginConfig) -> anyhow::Result<()> {
        write_private(&self.config_dir, &self.config_path(name), &(serde_json::to_string_pretty(config)? + "\n"))
    }

}

/// Settings saved in the config, completed with environment overrides.
pub(super) fn stored(manifest: &PluginManifest, config: &PluginConfig) -> Vec<(String, DataValue)> {
    stored_with_env(manifest, config, &|name| std::env::var(name).ok())
}

/// `TICKNODE_PLUGIN_<PLUGIN>_<KEY>`, both uppercased with every non-alphanumeric character turned into `_`.
pub(super) fn env_var_name(plugin: &str, key: &str) -> String {
    let norm = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect::<String>();
    format!("TICKNODE_PLUGIN_{}_{}", norm(plugin), norm(key))
}

/// Parses an environment value according to the setting kind (`None` if it does not fit).
fn parse_env_value(plugin: &str, def: &SettingDefinition, text: &str) -> Option<DataValue> {
    let value = match &def.kind {
        SettingKind::Number => DataValue::from(text.trim().parse::<f64>().ok().filter(|n| n.is_finite())?),
        SettingKind::Bool => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => DataValue::Bool(true),
            "false" | "0" | "no" | "off" => DataValue::Bool(false),
            _ => return None,
        },
        _ => DataValue::String(text.to_string()),
    };
    def.check(plugin, &value).ok()?;
    Some(value)
}

/// The stored settings plus values from `TICKNODE_PLUGIN_<PLUGIN>_<KEY>` for keys the config has no value for
/// (stored values win; an empty string counts as no value). `lookup` reads the environment.
pub(super) fn stored_with_env(manifest: &PluginManifest, config: &PluginConfig, lookup: &dyn Fn(&str) -> Option<String>) -> Vec<(String, DataValue)> {
    let mut values: Vec<(String, DataValue)> = config.settings.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    for def in &manifest.settings {
        let unset = config.settings.get(&def.key).is_none_or(|v| matches!(v, DataValue::String(s) if s.is_empty()));
        if !unset {
            continue;
        }
        let var = env_var_name(&manifest.name, &def.key);
        let Some(text) = lookup(&var).filter(|t| !t.is_empty()) else { continue };
        match parse_env_value(&manifest.name, def, &text) {
            Some(value) => {
                values.retain(|(k, _)| *k != def.key);
                values.push((def.key.clone(), value));
            }
            None => warn!(variable = %var, "Ignoring the environment value: it does not fit the setting"),
        }
    }
    values
}

/// Atomic write (tmp + rename) with mode 0600: the file may contain tokens.
pub(crate) fn write_private(dir: &Path, path: &Path, contents: &str) -> anyhow::Result<()> {
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
