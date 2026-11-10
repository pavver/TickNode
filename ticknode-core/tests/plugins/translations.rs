// --- Translations shipped by plugins ---

fn locale_text(translations: &Translations, locale: &str) -> String {
    translations
        .iter()
        .find(|(l, _)| l == locale)
        .map(|(_, t)| t.clone())
        .unwrap_or_default()
}

#[test]
fn plugin_translations_are_validated_and_delivered() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    // A disabled plugin contributes nothing; the core resources are always there
    let before = engine.translations();
    assert!(locale_text(&before, "uk").contains("node-const-title"));
    assert!(!locale_text(&before, "uk").contains("node-test-greeter"));

    engine
        .enable_plugin("test_plugin", &test_settings("Hello", "good"))
        .unwrap();
    let after = engine.translations();
    let uk = locale_text(&after, "uk");
    assert!(uk.contains("node-const-title"), "core resource first");
    assert!(
        uk.contains("node-test-greeter-description") && uk.contains("type-test-mode-value-manual"),
        "{uk}"
    );
    assert!(uk.contains("test-plugin-error-bad-token"));
    assert!(uk.contains("setting-test-plugin-mode-option-loud"));
    // `node-math-add-title` is not the plugin's to define: dropped, the core text stays
    assert!(!uk.contains("HIJACKED"), "{uk}");
    assert!(
        uk.contains("node-math-add-title"),
        "the core definition is kept"
    );
    // A resource with a syntax error is rejected as a whole; an unknown locale is passed on
    assert!(
        after.iter().all(|(l, _)| l != "de"),
        "{:?}",
        after.iter().map(|(l, _)| l).collect::<Vec<_>>()
    );
    assert!(locale_text(&after, "zz").contains("Counter (zz)"));

    // Disabling removes the plugin's resources again
    engine.disable_plugin("test_plugin").unwrap();
    let off = engine.translations();
    assert!(!locale_text(&off, "uk").contains("node-test-greeter"));
    assert!(locale_text(&off, "zz").is_empty());
}

#[test]
fn configure_errors_carry_translatable_messages() {
    let Some(mut engine) = engine_with_disabled_test_plugin() else {
        return;
    };
    engine
        .enable_plugin("test_plugin", &test_settings("x", "good"))
        .unwrap();

    // Own namespace: key and arguments pass through
    let e = engine
        .configure_plugin("test_plugin", &test_settings("x", "bad"))
        .unwrap_err();
    assert_eq!(
        (e.key.as_str(), e.fallback.as_str()),
        ("test-plugin-error-bad-token", "Invalid token")
    );
    assert_eq!(e.args, vec![("length".to_string(), "3".to_string())]);

    // A key outside the plugin namespace is dropped, the English fallback stays
    let e = engine
        .configure_plugin("test_plugin", &test_settings("x", "foreign"))
        .unwrap_err();
    assert_eq!((e.key.as_str(), e.fallback.as_str()), ("", "Foreign key"));

    // A plain `String` error has no key
    let e = engine
        .configure_plugin("test_plugin", &test_settings("x", "plain"))
        .unwrap_err();
    assert_eq!((e.key.as_str(), e.fallback.as_str()), ("", "Plain error"));
}

#[test]
fn engine_errors_have_ui_messages() {
    let e = EngineError::UnknownNodeType("x/y".into()).ui_message();
    assert_eq!(
        (e.key.as_str(), e.args.clone()),
        (
            "error-unknown-node-type",
            vec![("type".to_string(), "x/y".to_string())]
        )
    );
    assert_eq!(e.fallback, "Unknown node type: x/y");
}

