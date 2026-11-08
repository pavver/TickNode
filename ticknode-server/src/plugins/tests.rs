use super::discovery::plugin_paths;
use super::*;
use super::config::{env_var_name, stored_with_env};

#[test]
fn user_plugin_overrides_bundled_one() {
    let (user, bundled) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for (dir, name) in [(&user, "a.wasm"), (&bundled, "a.wasm"), (&bundled, "b.wasm"), (&bundled, "notes.txt")] {
        std::fs::write(dir.path().join(name), b"").unwrap();
    }
    let paths = plugin_paths(user.path(), Some(bundled.path()));
    assert_eq!(paths, vec![user.path().join("a.wasm"), bundled.path().join("b.wasm")]);
    assert_eq!(plugin_paths(user.path(), None), vec![user.path().join("a.wasm")]);
}

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

fn env_manifest() -> PluginManifest {
    PluginManifest::new("home_assistant", "1", "t")
        .setting(SettingDefinition::url("base-url", "URL"))
        .setting(SettingDefinition::secret("token", "Token").required())
        .setting(SettingDefinition::number("retries", "Retries"))
        .setting(SettingDefinition::bool("debug", "Debug"))
}

#[test]
fn environment_supplies_missing_settings_only() {
    assert_eq!(env_var_name("home_assistant", "base-url"), "TICKNODE_PLUGIN_HOME_ASSISTANT_BASE_URL");
    let manifest = env_manifest();
    let env = |name: &str| match name {
        "TICKNODE_PLUGIN_HOME_ASSISTANT_TOKEN" => Some("from-env".to_string()),
        "TICKNODE_PLUGIN_HOME_ASSISTANT_BASE_URL" => Some("ws://env/api/websocket".to_string()),
        "TICKNODE_PLUGIN_HOME_ASSISTANT_RETRIES" => Some("3".to_string()),
        "TICKNODE_PLUGIN_HOME_ASSISTANT_DEBUG" => Some("maybe".to_string()),
        _ => None,
    };
    let mut config = PluginConfig::default();
    config.settings.insert("base-url".into(), DataValue::String("ws://stored/api/websocket".into()));
    let effective = effective_settings(&manifest, &stored_with_env(&manifest, &config, &env));
    let get = |k: &str| effective.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    // The stored value wins, missing ones come from the environment, an invalid one is ignored.
    assert_eq!(get("base-url"), Some(DataValue::String("ws://stored/api/websocket".into())));
    assert_eq!(get("token"), Some(DataValue::String("from-env".into())));
    assert_eq!(get("retries"), Some(DataValue::from(3.0)));
    assert_eq!(get("debug"), None);
    // An empty stored string counts as unset.
    config.settings.insert("token".into(), DataValue::String(String::new()));
    let effective = effective_settings(&manifest, &stored_with_env(&manifest, &config, &env));
    assert!(effective.contains(&("token".to_string(), DataValue::String("from-env".into()))));
}

fn fixture(dir: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    std::fs::create_dir_all(dir.join("plugins")).unwrap();
    std::fs::copy(source, dir.join("plugins/test.wasm")).expect("build test-plugin for wasm32-wasip2 first");
}

#[test]
fn discovery_records_invalid_components_and_user_override() {
    let dir = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("plugins")).unwrap();
    std::fs::write(dir.path().join("plugins/bad.wasm"), b"invalid").unwrap();
    std::fs::write(bundled.path().join("bad.wasm"), b"invalid").unwrap();
    let mut engine = TickEngine::new();
    let mut plugins = Plugins::new(dir.path()).with_bundled_dir(Some(bundled.path().into()));
    plugins.load_all(&mut engine);
    let infos = plugins.infos(&engine);
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].name, "bad");
    assert_eq!(infos[0].state, PluginState::Error);
    assert!(infos[0].error.as_ref().unwrap().fallback.starts_with("Invalid component:"));
    assert_eq!(plugins.set_enabled(&mut engine, "bad", true).err(), infos[0].error);
}

#[test]
fn plugin_settings_and_enable_state_survive_discovery() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    let mut engine = TickEngine::new();
    let mut plugins = Plugins::new(dir.path());
    plugins.load_all(&mut engine);
    assert_eq!(plugins.info(&engine, "test_plugin").unwrap().state, PluginState::Enabled);
    plugins.configure(&mut engine, "test_plugin", vec![
        ("token".into(), SettingUpdate::Set(DataValue::from("secret"))),
        ("greeting".into(), SettingUpdate::Set(DataValue::from("Hola"))),
    ]).unwrap();
    let info = plugins.info(&engine, "test_plugin").unwrap();
    assert_eq!(info.settings_error, None);
    assert!(info.values.contains(&("token".into(), SettingValueView::Secret { is_set: true })));
    assert!(info.values.contains(&("greeting".into(), SettingValueView::Value(DataValue::from("Hola")))));
    plugins.configure(&mut engine, "test_plugin", vec![("token".into(), SettingUpdate::Keep)]).unwrap();
    assert_eq!(plugins.read_config("test_plugin").unwrap().settings["token"], DataValue::from("secret"));
    plugins.configure(&mut engine, "test_plugin", vec![("token".into(), SettingUpdate::Clear)]).unwrap();
    assert!(!plugins.read_config("test_plugin").unwrap().settings.contains_key("token"));
    assert!(plugins.info(&engine, "test_plugin").unwrap().settings_error.is_some());
    assert_eq!(plugins.configure(&mut engine, "test_plugin", vec![("unknown".into(), SettingUpdate::Keep)]).unwrap_err().key, "error-setting-unknown");
    plugins.set_enabled(&mut engine, "test_plugin", false).unwrap();
    let mut restored = Plugins::new(dir.path());
    let mut restored_engine = TickEngine::new();
    restored.load_all(&mut restored_engine);
    assert_eq!(restored.info(&restored_engine, "test_plugin").unwrap().state, PluginState::Disabled);
    restored.set_enabled(&mut restored_engine, "test_plugin", true).unwrap();
    assert_eq!(restored.info(&restored_engine, "test_plugin").unwrap().state, PluginState::Enabled);
}

#[test]
fn corrupt_plugin_config_stays_visible_until_explicit_save() {
    let dir = tempfile::tempdir().unwrap();
    fixture(dir.path());
    std::fs::create_dir(dir.path().join("plugin-config")).unwrap();
    std::fs::write(dir.path().join("plugin-config/test_plugin.json"), b"broken").unwrap();
    let mut engine = TickEngine::new();
    let mut plugins = Plugins::new(dir.path());
    plugins.load_all(&mut engine);
    let info = plugins.info(&engine, "test_plugin").unwrap();
    assert_eq!(info.state, PluginState::Error);
    assert_eq!(info.error.unwrap().key, CONFIG_CORRUPT_KEY);
    assert!(!engine.plugin_active("test_plugin"));
    assert_eq!(std::fs::read_to_string(dir.path().join("plugin-config/test_plugin.json")).unwrap(), "broken");
    plugins.configure(&mut engine, "test_plugin", vec![("greeting".into(), SettingUpdate::Set(DataValue::from("Saved")))]).unwrap();
    assert_eq!(plugins.info(&engine, "test_plugin").unwrap().state, PluginState::Disabled);
    assert_eq!(plugins.read_config("test_plugin").unwrap().settings["greeting"], DataValue::from("Saved"));
}
