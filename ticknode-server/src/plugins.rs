//! Плагіни на сервері: пошук `data/plugins/*.wasm`, стан «увімкнено / вимкнено / помилка» і
//! налаштування, що зберігаються у `data/plugin-config/<назва>.json` (там можуть бути секрети:
//! файл пишеться з правами 0600, а в редактор секрети не надсилаються).
//!
//! Плагін без файлу конфігурації вважається увімкненим з налаштуваннями за замовчуванням.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use ticknode_core::{PluginChange, TickEngine, effective_settings};
use ticknode_shared::*;
use tracing::{info, warn};

/// Збережений стан одного плагіна.
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
    /// Не завантажився, не ввімкнувся чи пошкоджений файл конфігурації.
    error: Option<String>,
    /// Компонент завантажено в рушій (є маніфест).
    loaded: bool,
    /// Для плагінів, що не завантажились (маніфесту немає).
    version: String,
    author: String,
}

/// Результат вмикання/вимкнення: зміни графа й необов'язкове попередження ініціатору.
pub struct Outcome {
    pub change: PluginChange,
    pub warning: Option<String>,
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
        let text = std::fs::read_to_string(&path).with_context(|| format!("не вдалось прочитати {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("некоректний JSON у {}", path.display()))
    }

    fn write_config(&self, name: &str, config: &PluginConfig) -> anyhow::Result<()> {
        write_private(&self.config_dir, &self.config_path(name), &(serde_json::to_string_pretty(config)? + "\n"))
    }

    /// Знаходить `*.wasm`, читає маніфести й вмикає плагіни, які не вимкнені в конфігурації.
    pub fn load_all(&mut self, engine: &mut TickEngine) {
        let Ok(dir) = std::fs::read_dir(&self.plugins_dir) else { return };
        let mut paths: Vec<_> = dir.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths.into_iter().filter(|p| p.extension().is_some_and(|e| e == "wasm")) {
            let file = path.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
            let stem = path.file_stem().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
            let loaded = std::fs::read(&path)
                .map_err(|e| format!("Не вдалось прочитати плагін: {e}"))
                .and_then(|bytes| engine.add_plugin(&bytes, Some(&self.plugin_data), |m| m.requested_capabilities.clone()));
            let manifest = match loaded {
                Ok(m) => m,
                Err(e) => {
                    warn!(path = %path.display(), "Не вдалось завантажити плагін: {}", e);
                    let key = if self.entries.contains_key(&stem) { file.clone() } else { stem };
                    self.entries.insert(key, Entry { file, config: PluginConfig::default(), error: Some(e), loaded: false, version: String::new(), author: String::new() });
                    continue;
                }
            };
            let (config, mut error) = match self.read_config(&manifest.name) {
                Ok(c) => (c, None),
                Err(e) => {
                    warn!(name = %manifest.name, "Конфігурацію плагіна не прочитано: {:#}", e);
                    (PluginConfig { enabled: false, ..PluginConfig::default() }, Some(format!("Конфігурація плагіна пошкоджена: {e:#}. Збереження налаштувань чи вмикання перезапише її")))
                }
            };
            if config.enabled && error.is_none() {
                let settings = effective_settings(&manifest, &stored(&config));
                match engine.enable_plugin(&manifest.name, &settings) {
                    Ok(_) => info!(name = %manifest.name, version = %manifest.version, capabilities = ?manifest.requested_capabilities, "Плагін завантажено"),
                    Err(e) => {
                        warn!(name = %manifest.name, "Не вдалось увімкнути плагін: {}", e);
                        error = Some(e);
                    }
                }
                if let Some(e) = engine.plugin_settings_error(&manifest.name) {
                    warn!(name = %manifest.name, "Помилка налаштувань плагіна: {}", e);
                }
            } else if error.is_none() {
                info!(name = %manifest.name, "Плагін вимкнено в конфігурації");
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
        // Секрет у повідомлення не потрапляє ніколи: лише чи задано
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

    /// Вмикає чи вимикає плагін на льоту. `Err` — відмова (залежності, конфлікт типів): стан не змінено.
    pub fn set_enabled(&mut self, engine: &mut TickEngine, name: &str, enabled: bool) -> Result<Outcome, String> {
        let entry = self.entries.get(name).ok_or_else(|| format!("Плагін {name} не знайдено"))?;
        if !entry.loaded {
            return Err(format!("Плагін {name} не завантажено: {}", entry.error.as_deref().unwrap_or("невідома помилка")));
        }
        let manifest = engine.plugin_manifest(name).cloned().ok_or_else(|| format!("Плагін {name} не завантажено"))?;
        let mut config = entry.config.clone();
        config.enabled = enabled;
        let change = match (enabled, engine.plugin_active(name)) {
            (true, false) => {
                let settings = effective_settings(&manifest, &stored(&config));
                match engine.enable_plugin(name, &settings) {
                    Ok(change) => change,
                    Err(e) => {
                        // Стан «помилка» лишається видимим у панелі, доки вмикання не вдасться
                        self.entries.get_mut(name).expect("запис щойно знайдено").error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
            (false, true) => engine.disable_plugin(name)?,
            _ => PluginChange::default(),
        };
        let entry = self.entries.get_mut(name).expect("запис щойно знайдено");
        entry.error = None;
        entry.config = config.clone();
        let warning = self.write_config(name, &config)
            .err()
            .map(|e| format!("Стан плагіна змінено, але конфігурацію не збережено: {e:#}"));
        if let Some(w) = &warning {
            warn!("{w}");
        }
        Ok(Outcome { change, warning })
    }

    /// Зберігає налаштування й передає їх плагіну (якщо він увімкнений). Відповідь `configure`
    /// не є помилкою цієї дії: вона лишається в `PluginInfo::settings_error`.
    pub fn configure(&mut self, engine: &mut TickEngine, name: &str, values: Vec<(String, SettingUpdate)>) -> Result<(), String> {
        let entry = self.entries.get(name).ok_or_else(|| format!("Плагін {name} не знайдено"))?;
        let manifest = engine.plugin_manifest(name).filter(|_| entry.loaded).cloned()
            .ok_or_else(|| format!("Плагін {name} не завантажено"))?;
        let mut config = entry.config.clone();
        // Пошкоджена конфігурація замінюється цим збереженням
        if entry.error.as_deref().is_some_and(|e| e.starts_with("Конфігурація плагіна пошкоджена")) {
            config = PluginConfig { enabled: false, ..PluginConfig::default() };
        }
        for (key, update) in values {
            let def = manifest.settings.iter().find(|d| d.key == key).ok_or_else(|| format!("Невідоме налаштування «{key}»"))?;
            match update {
                SettingUpdate::Set(value) => {
                    def.check(&value)?;
                    config.settings.insert(key, value);
                }
                SettingUpdate::Keep => {}
                SettingUpdate::Clear => {
                    config.settings.remove(&key);
                }
            }
        }
        self.write_config(name, &config).map_err(|e| format!("Не вдалось зберегти налаштування: {e:#}"))?;
        let entry = self.entries.get_mut(name).expect("запис щойно знайдено");
        if entry.error.as_deref().is_some_and(|e| e.starts_with("Конфігурація плагіна пошкоджена")) {
            entry.error = None;
        }
        entry.config = config.clone();
        if let Err(e) = engine.configure_plugin(name, &effective_settings(&manifest, &stored(&config))) {
            warn!(name, "Помилка налаштувань плагіна: {e}");
        }
        Ok(())
    }
}

fn stored(config: &PluginConfig) -> Vec<(String, DataValue)> {
    config.settings.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Атомарний запис (tmp + rename) з правами 0600: у файлі можуть бути токени.
fn write_private(dir: &Path, path: &Path, contents: &str) -> anyhow::Result<()> {
    use std::io::Write;
    std::fs::create_dir_all(dir).with_context(|| format!("не вдалось створити {}", dir.display()))?;
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
    let mut file = options.open(&tmp).with_context(|| format!("не вдалось записати {}", tmp.display()))?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&tmp, path).with_context(|| format!("не вдалось замінити {}", path.display()))?;
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
        // Повторний запис не лишає тимчасових файлів
        plugins.write_config("p", &config).unwrap();
        assert_eq!(std::fs::read_dir(dir.path().join("plugin-config")).unwrap().count(), 1);
        // Без файлу — увімкнений з типовими налаштуваннями
        assert_eq!(plugins.read_config("none").unwrap(), PluginConfig::default());
    }
}
