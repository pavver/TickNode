use super::*;

#[test]
fn saving_validates_the_whole_form_and_keeps_schema_order_and_secret_intent() {
    let defs = [SettingDefinition::text("name", "Name"), SettingDefinition::secret("token", "Token"), SettingDefinition::number("port", "Port")];
    let mut drafts = BTreeMap::from([("name".into(), "Home".into()), ("port".into(), "8123".into())]);
    assert_eq!(form_updates("p", &defs, &drafts, &BTreeSet::new()).unwrap(), [
        ("name".into(), SettingUpdate::Set(DataValue::String("Home".into()))),
        ("token".into(), SettingUpdate::Keep),
        ("port".into(), SettingUpdate::Set(DataValue::Number(NumberValue::Int(8123)))),
    ]);
    let cleared = BTreeSet::from(["token".into()]);
    assert_eq!(form_updates("p", &defs, &drafts, &cleared).unwrap()[1].1, SettingUpdate::Clear);
    drafts.insert("port".into(), "invalid".into());
    assert!(form_updates("p", &defs, &drafts, &cleared).unwrap_err().contains("Port"));
}

#[test]
fn plugin_states_and_missing_manifest_have_distinct_labels() {
    i18n::with_locale(i18n::Locale::En, || {
        for (state, control, key) in [(PluginState::Enabled, ControlState::Success, "plugins-state-enabled"),
            (PluginState::Disabled, ControlState::Idle, "plugins-state-disabled"),
            (PluginState::Error, ControlState::Error, "plugins-state-error")] {
            assert_eq!(state_chip(state), (control, t!(key)));
        }
    });
    let mut info = PluginInfo {
        name: "p".into(), version: String::new(), author: String::new(), file: "p.wasm".into(),
        state: PluginState::Error, error: None, settings_error: None, capabilities: vec![], settings: vec![], values: vec![],
    };
    assert_eq!(plugin_caption(&info), "p.wasm");
    info.version = "1.2".into();
    assert_eq!(plugin_caption(&info), "v1.2 · ");
    info.version.clear();
    info.author = "Author".into();
    assert_eq!(plugin_caption(&info), "v · Author");
    info.author.clear();
    info.capabilities.push(Capability::Storage);
    assert_eq!(plugin_caption(&info), "v · ");
    info.capabilities.clear();
    info.settings.push(SettingDefinition::text("key", "Key"));
    assert_eq!(plugin_caption(&info), "v · ");
}

#[test]
fn capability_labels_keep_wildcards_and_domain_order() {
    i18n::with_locale(i18n::Locale::En, || {
        assert_eq!(capability_text(&Capability::Http { allowed_domains: vec!["*".into()] }), t!("plugins-cap-http-any"));
        assert_eq!(capability_text(&Capability::Http { allowed_domains: vec!["b".into(), "a".into()] }), t!("plugins-cap-http-domains", domains = "b, a"));
        assert_eq!(capability_text(&Capability::WebSocket { allowed_hosts: vec!["*".into()] }), t!("plugins-cap-ws-any"));
        assert_eq!(capability_text(&Capability::WebSocket { allowed_hosts: vec!["host".into()] }), t!("plugins-cap-ws-hosts", hosts = "host"));
        assert_eq!(capability_text(&Capability::Storage), t!("plugins-cap-storage"));
    });
}

#[test]
fn form_values_map_to_updates() {
    let text = SettingDefinition::text("greeting", "Greeting");
    let secret = SettingDefinition::secret("token", "Token");
    let number = SettingDefinition::number("port", "Port");
    let flag = SettingDefinition::bool("tls", "TLS");
    assert_eq!(update_of("p", &text, "hi", false), Ok(SettingUpdate::Set(DataValue::String("hi".into()))));
    assert_eq!(update_of("p", &text, "", false), Ok(SettingUpdate::Clear));
    // Secret: empty field means "keep unchanged", explicit clear means Clear
    assert_eq!(update_of("p", &secret, "", false), Ok(SettingUpdate::Keep));
    assert_eq!(update_of("p", &secret, "", true), Ok(SettingUpdate::Clear));
    assert_eq!(update_of("p", &secret, "abc", false), Ok(SettingUpdate::Set(DataValue::String("abc".into()))));
    assert_eq!(update_of("p", &number, "8123", false), Ok(SettingUpdate::Set(DataValue::Number(NumberValue::Int(8123)))));
    assert!(update_of("p", &number, "abc", false).is_err());
    assert_eq!(update_of("p", &number, " ", false), Ok(SettingUpdate::Clear));
    assert_eq!(update_of("p", &flag, "true", false), Ok(SettingUpdate::Set(DataValue::Bool(true))));
    assert_eq!(update_of("p", &flag, "", false), Ok(SettingUpdate::Set(DataValue::Bool(false))));
}

#[test]
fn secrets_never_prefill_the_form() {
    assert_eq!(draft_of(&SettingValueView::Secret { is_set: true }), "");
    assert_eq!(draft_of(&SettingValueView::Value(DataValue::Bool(true))), "true");
    assert_eq!(draft_of(&SettingValueView::Value(DataValue::String("x".into()))), "x");
}
