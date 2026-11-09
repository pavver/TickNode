use super::*;
use crate::plugin::options::sanitize_options;

fn engine() -> Option<TickEngine> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(&path) else {
        assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(), "{} not found", path.display());
        return None;
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    Some(engine)
}

fn option() -> wit::PropertyOption {
    wit::PropertyOption { value: "value".into(), label: "Label".into(), detail: "Detail".into(), icon: Some("mdi:test".into()) }
}

#[test]
fn core_c_option_boundaries_preserve_valid_options_and_report_only_drops() {
    let source = "s".repeat(MAX_OPTION_STRING_LEN);
    let (accepted, warnings) = sanitize_options(true, &source, vec![option(); MAX_OPTIONS_PER_SOURCE]);
    assert_eq!(accepted.len(), MAX_OPTIONS_PER_SOURCE);
    assert_eq!(accepted[0].value, "value");
    assert!(warnings.is_empty(), "{warnings:?}");
    for source in ["".into(), "s".repeat(MAX_OPTION_STRING_LEN + 1)] {
        let (accepted, warnings) = sanitize_options(true, &source, vec![option()]);
        assert!(accepted.is_empty());
        assert_eq!(warnings, ["Option source name is empty or too long: ignored"]);
    }
    let (accepted, warnings) = sanitize_options(true, "ok", vec![option(); MAX_OPTIONS_PER_SOURCE + 1]);
    assert_eq!(accepted.len(), MAX_OPTIONS_PER_SOURCE);
    assert_eq!(warnings, [format!("Source \"ok\" has {} options, keeping the first {MAX_OPTIONS_PER_SOURCE}", MAX_OPTIONS_PER_SOURCE + 1)]);
    let mut bad = option();
    bad.value.clear();
    let (accepted, warnings) = sanitize_options(true, "ok", vec![bad, option()]);
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].value, "value");
    assert_eq!(warnings, [format!("Source \"ok\": dropped 1 option(s) with an empty value or a string longer than {MAX_OPTION_STRING_LEN} bytes")]);
}

#[test]
fn core_c_each_option_string_accepts_the_exact_byte_limit() {
    for field in 0..4 {
        let mut item = option();
        let target = match field { 0 => &mut item.value, 1 => &mut item.label, 2 => &mut item.detail, _ => item.icon.as_mut().unwrap() };
        // Multibyte text verifies that this is a byte limit rather than a character limit.
        *target = "é".repeat(MAX_OPTION_STRING_LEN / 2);
        let (accepted, warnings) = sanitize_options(true, "ok", vec![item.clone()]);
        assert_eq!(accepted.len(), 1, "field {field}");
        assert!(warnings.is_empty(), "{warnings:?}");
        let target = match field { 0 => &mut item.value, 1 => &mut item.label, 2 => &mut item.detail, _ => item.icon.as_mut().unwrap() };
        target.push('x');
        let (accepted, warnings) = sanitize_options(true, "ok", vec![item, option()]);
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].label, "Label");
        assert_eq!(warnings.len(), 1);
    }
}

#[test]
fn core_c_host_source_capacity_allows_updates_but_rejects_one_more_source() {
    use crate::plugin::bindings::ticknode::plugin::host::Host;
    let Some(engine) = engine() else { return };
    let mut rt = lock(&engine.plugins["test_plugin"].runtime);
    let (store, _) = rt.api().unwrap();
    let state = store.data_mut();
    lock(&state.options).clear();
    for i in 0..MAX_OPTION_SOURCES {
        state.set_options(format!("source-{i}"), vec![option()]);
    }
    state.set_options("overflow".into(), vec![option()]);
    let mut updated = option();
    updated.label = "Updated".into();
    state.set_options("source-0".into(), vec![updated]);
    let all = lock(&state.options).all("test_plugin");
    assert_eq!(all.len(), MAX_OPTION_SOURCES);
    assert!(!all.iter().any(|s| s.source == "overflow"));
    assert_eq!(all.iter().find(|s| s.source == "source-0").unwrap().options[0].label, "Updated");
}

#[test]
fn core_c_required_settings_match_their_own_key() {
    let manifest = PluginManifest::new("settings", "1", "test")
        .setting(SettingDefinition::text("optional", "Optional"))
        .setting(SettingDefinition::secret("token", "Token").required());
    let error = missing_required(&manifest, &[("optional".into(), "value".into())]).unwrap();
    assert_eq!(error.key, "error-setting-required");
    assert!(error.args.contains(&("key".into(), "token".into())));
    assert_eq!(missing_required(&manifest, &[("token".into(), "secret".into())]), None);
}

#[test]
fn core_c_dependency_error_distinguishes_one_and_multiple_dependents() {
    let Some(mut engine) = engine() else { return };
    let runtime = engine.plugins["test_plugin"].runtime.clone();
    let pins = vec![PinDefinition { id: PinId(0), name: "P".into(), pin_type: PinType::Data(DataType::Named("test.point".into())), direction: PinDirection::Input, required: false }];
    engine.plugins.insert("consumer".into(), consumer_slot(runtime.clone(), pins.clone(), vec![]));
    let error = engine.disable_plugin("test_plugin").unwrap_err();
    assert_eq!(error.fallback, "Plugin \"test_plugin\" cannot be disabled: its data types are used by \"consumer\"; disable it first");
    engine.plugins.insert("second".into(), consumer_slot(runtime, pins, vec![]));
    let error = engine.disable_plugin("test_plugin").unwrap_err();
    assert!(error.fallback.ends_with("; disable them first"), "{error}");
    assert!(engine.plugin_active("test_plugin"));
}

#[test]
fn core_c_output_tag_validation_uses_the_selected_output_not_a_neighbor() {
    let mut engine = TickEngine::new();
    engine.registry.register_owned("mine", Box::new(|id| Box::new(crate::CompareNode { id })));
    let node = NodeId::generate();
    engine.create_node(node, "compare", &[]).unwrap();
    // The validator is shared by node executions and plugin-level emissions.
    let mut definition = engine.node_definition(node).unwrap();
    definition.pins = vec![
        PinDefinition { id: PinId(1), name: "Other".into(), direction: PinDirection::Output, pin_type: PinType::Data(DataType::Named("other.point".into())), required: false },
        PinDefinition { id: PinId(2), name: "Point".into(), direction: PinDirection::Output, pin_type: PinType::Data(DataType::Named("mine.point".into())), required: false },
    ];
    let point = DataValue::Object { type_name: Some("mine.point".into()), fields: Default::default() };
    let (valid, errors) = validate_emission_outputs(&definition, vec![wit::Output::Data((2, encode_value(&point)))]);
    assert_eq!(valid, [NodeOutput::Data(PinId(2), point)]);
    assert!(errors.is_empty(), "{errors:?}");
    let wrong = DataValue::Object { type_name: Some("other.point".into()), fields: Default::default() };
    let (valid, errors) = validate_emission_outputs(&definition, vec![wit::Output::Data((2, encode_value(&wrong)))]);
    assert!(valid.is_empty());
    assert_eq!(errors.len(), 1);
}

#[test]
fn core_c_refreshed_plugin_definition_changes_scheduling_cost() {
    let Some(mut engine) = engine() else { return };
    let node = NodeId::generate();
    engine.create_node(node, "test/cost", &[]).unwrap();
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(engine.last_tick.spent, 1);
    // A node refresh can occur independently of the engine's explicit edit invalidation.
    engine.nodes.get_mut(&node).unwrap().set_property("cost", 7i64.into());
    engine.trigger_node(node, PinId(0));
    engine.tick();
    assert_eq!(engine.last_tick.spent, 7);
    assert!(engine.executed.iter().any(|t| t.node_id == node));
}

#[test]
fn core_c_plugin_tick_resumes_at_the_exact_backoff_deadline() {
    let Some(mut engine) = engine() else { return };
    engine.configure_plugin("test_plugin", &[("greeting".into(), "tick-clock".into()), ("token".into(), "t".into())]).unwrap();
    engine.tick();
    assert_eq!(engine.host_state.logs, [(NodeId(uuid::Uuid::nil()), "Plugin tick 1".into())]);
    engine.configure_plugin("test_plugin", &[("greeting".into(), "trap-tick".into()), ("token".into(), "t".into())]).unwrap();
    engine.host_state.current_tick = 99;
    engine.host_state.logs.clear();
    engine.tick();
    assert!(engine.host_state.logs.contains(&(NodeId(uuid::Uuid::nil()), "Plugin tick 100".into())));
    assert!(engine.host_state.logs.iter().any(|(_, log)| log.contains("crashed in plugin-tick")));
    engine.host_state.logs.clear();
    for _ in 0..49 { engine.tick(); }
    assert_eq!(engine.host_state.current_tick, 149);
    assert!(engine.host_state.logs.is_empty(), "no plugin code runs before the deadline");
    engine.tick();
    assert!(engine.host_state.logs.contains(&(NodeId(uuid::Uuid::nil()), "Plugin tick 150".into())));
    assert!(engine.host_state.logs.iter().any(|(_, log)| log.contains("crashed in plugin-tick")));
}
