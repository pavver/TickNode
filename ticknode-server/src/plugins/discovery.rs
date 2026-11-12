use super::*;

impl Plugins {
    /// Discovers `*.wasm` files, reads manifests, and enables plugins that are not disabled in config.
    pub fn load_all(&mut self, engine: &mut TickEngine) {
        for path in plugin_paths(&self.plugins_dir, self.bundled_dir.as_deref()) {
            self.load_path(engine, &path);
        }
    }

    fn load_path(&mut self, engine: &mut TickEngine, path: &Path) {
        let file = path
            .file_name()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        let stem = path
            .file_stem()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        let loaded = std::fs::read(&path)
            .map_err(|e| UiMessage::plain(format!("Failed to read the plugin: {e}")))
            .and_then(|bytes| {
                engine.add_plugin(&bytes, Some(&self.plugin_data), |m| {
                    m.requested_capabilities.clone()
                })
            });
        let manifest = match loaded {
            Ok(m) => m,
            Err(e) => {
                warn!(path = %path.display(), "Failed to load the plugin: {}", e);
                let key = if self.entries.contains_key(&stem) {
                    file.clone()
                } else {
                    stem
                };
                self.entries.insert(
                    key,
                    Entry {
                        file,
                        config: PluginConfig::default(),
                        error: Some(e),
                        loaded: false,
                        version: String::new(),
                        author: String::new(),
                    },
                );
                return;
            }
        };
        let (config, error) = self.discovered_state(engine, &manifest);
        self.entries.insert(
            manifest.name.clone(),
            Entry {
                file,
                config,
                error,
                loaded: true,
                version: manifest.version,
                author: manifest.author,
            },
        );
    }

    fn discovered_state(
        &self,
        engine: &mut TickEngine,
        manifest: &PluginManifest,
    ) -> (PluginConfig, Option<UiMessage>) {
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
            error = enable_discovered(engine, manifest, &config);
        } else if error.is_none() {
            info!(name = %manifest.name, "Plugin is disabled in the config");
        }
        (config, error)
    }
}

/// `*.wasm` files of the user directory plus the bundled one, sorted by path; on a file name clash the user's file wins.
pub(super) fn plugin_paths(user_dir: &Path, bundled_dir: Option<&Path>) -> Vec<PathBuf> {
    let scan = |dir: &Path| -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "wasm"))
            .collect();
        paths.sort();
        paths
    };
    let mut paths = scan(user_dir);
    if let Some(bundled) = bundled_dir {
        for path in scan(bundled) {
            if paths.iter().any(|p| p.file_name() == path.file_name()) {
                info!(path = %path.display(), "A plugin with the same file name exists in the data directory, skipping the bundled one");
            } else {
                paths.push(path);
            }
        }
    }
    paths
}

fn enable_discovered(
    engine: &mut TickEngine,
    manifest: &PluginManifest,
    config: &PluginConfig,
) -> Option<UiMessage> {
    let mut error = None;
    let settings = effective_settings(manifest, &stored(manifest, config));
    match engine.enable_plugin(&manifest.name, &settings) {
        Ok(_) => {
            info!(name = %manifest.name, version = %manifest.version, capabilities = ?manifest.requested_capabilities, "Plugin loaded")
        }
        Err(e) => {
            warn!(name = %manifest.name, "Failed to enable the plugin: {}", e);
            error = Some(e);
        }
    }
    if let Some(e) = engine.plugin_settings_error(&manifest.name) {
        warn!(name = %manifest.name, "Plugin settings error: {}", e);
    }
    error
}
