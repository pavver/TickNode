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
