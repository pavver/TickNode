// --- Plugin settings (configure) ---

fn test_settings(greeting: &str, token: &str) -> Vec<(String, DataValue)> {
    vec![
        ("greeting".into(), DataValue::String(greeting.into())),
        ("token".into(), DataValue::String(token.into())),
    ]
}

/// Engine with test_plugin loaded but disabled.
fn engine_with_disabled_test_plugin() -> Option<TickEngine> {
    let bytes = plugin_bytes("test_plugin")?;
    let mut engine = TickEngine::new();
    engine.add_plugin(&bytes, None, |_| vec![]).unwrap();
    Some(engine)
}

fn greet(engine: &mut TickEngine, node: NodeId) -> Option<DataValue> {
    engine.trigger_node(node, PinId(0));
    engine.tick();
    output(engine, node, 1)
}

#[test]
fn manifest_has_settings_schema() {
    let Some(engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    let manifest = engine.plugin_manifest("test_plugin").unwrap();
    let keys: Vec<_> = manifest
        .settings
        .iter()
        .map(|s| (s.key.as_str(), s.required))
        .collect();
    assert_eq!(
        keys,
        [
            ("greeting", false),
            ("token", true),
            ("mode", false),
            ("ws_url", false),
            ("http_url", false),
            ("http_count", false),
            ("http_cancel", false),
            ("http_tick", false),
        ]
    );
    assert_eq!(
        manifest.settings[0].default,
        Some(DataValue::String("Hello".into()))
    );
    assert!(matches!(&manifest.settings[2].kind, SettingKind::Select(o) if o.len() == 2));
    // Added but not enabled: types are not registered yet
    assert!(!engine.plugin_active("test_plugin"));
    assert!(engine.registry.get_definition("test/greeter").is_none());
}

#[test]
fn configure_is_called_and_its_error_is_reported() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    let manifest = engine.plugin_manifest("test_plugin").unwrap().clone();

    // Enabling without token: configure rejects it, but the plugin remains operational
    let defaults = effective_settings(&manifest, &[]);
    engine.enable_plugin("test_plugin", &defaults).unwrap();
    assert_eq!(
        engine
            .plugin_settings_error("test_plugin")
            .map(|e| e.key)
            .as_deref(),
        Some("test-plugin-error-no-token")
    );
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    assert_eq!(
        greet(&mut engine, greeter),
        Some(DataValue::String("Hello|quiet|no-token".into()))
    );

    // New settings are applied without a restart
    engine
        .configure_plugin(
            "test_plugin",
            &effective_settings(&manifest, &test_settings("Hello", "good")),
        )
        .unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
    assert_eq!(
        greet(&mut engine, greeter),
        Some(DataValue::String("Hello|quiet|token".into()))
    );

    // Plugin response is returned as an error and recorded
    let err = engine
        .configure_plugin("test_plugin", &test_settings("x", "bad"))
        .unwrap_err();
    assert_eq!(err.key, "test-plugin-error-bad-token");
    assert_eq!(err.args, vec![("length".to_string(), "3".to_string())]);
    assert_eq!(err.fallback, "Invalid token");
    assert_eq!(
        engine
            .plugin_settings_error("test_plugin")
            .map(|e| e.fallback),
        Some("Invalid token".to_string())
    );
    assert!(engine.plugin_active("test_plugin"));

    // Fixing the setting clears the error
    engine
        .configure_plugin("test_plugin", &test_settings("x", "ok"))
        .unwrap();
    assert_eq!(engine.plugin_settings_error("test_plugin"), None);
}

#[test]
fn settings_survive_plugin_crash() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    engine
        .enable_plugin("test_plugin", &test_settings("Hello", "good"))
        .unwrap();
    let greeter = NodeId::generate();
    engine.create_node(greeter, "test/greeter", &[]).unwrap();
    let crash = NodeId::generate();
    engine.create_node(crash, "test/crash", &[]).unwrap();
    assert_eq!(
        greet(&mut engine, greeter),
        Some(DataValue::String("Hello|quiet|token".into()))
    );

    engine.trigger_node(crash, PinId(0));
    engine.tick();
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(n, m)| *n == crash && m.contains("Plugin crashed"))
    );

    // Instance recreated: configure was re-run before recreating the node
    assert_eq!(
        greet(&mut engine, greeter),
        Some(DataValue::String("Hello|quiet|token".into()))
    );
}

