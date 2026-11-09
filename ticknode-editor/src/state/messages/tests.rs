use super::*;

fn plugin(name: &str, state: PluginState) -> PluginInfo {
    PluginInfo { name: name.into(), state, version: "1".into(), author: String::new(), file: format!("{name}.wasm"),
        error: None, settings_error: None, capabilities: vec![], settings: vec![], values: vec![], resources: vec![] }
}

#[test]
fn plugin_updates_replace_existing_state_and_sort_only_new_entries() {
    let mut plugins = vec![plugin("z", PluginState::Enabled), plugin("a", PluginState::Enabled)];
    update_plugin(&mut plugins, plugin("z", PluginState::Disabled));
    assert_eq!(plugins, [plugin("z", PluginState::Disabled), plugin("a", PluginState::Enabled)]);
    update_plugin(&mut plugins, plugin("m", PluginState::Error));
    assert_eq!(plugins, [plugin("a", PluginState::Enabled), plugin("m", PluginState::Error), plugin("z", PluginState::Disabled)]);
}

#[test]
fn empty_options_remove_only_the_matching_plugin_source() {
    let mut all = PropertyOptions::new();
    let option = PropertyOption { value: "light.kitchen".into(), label: "Kitchen".into(), detail: String::new(), icon: None };
    update_options(&mut all, "ha".into(), "entities".into(), vec![option.clone()]);
    update_options(&mut all, "other".into(), "entities".into(), vec![option.clone()]);
    let changed = PropertyOption { label: "Renamed kitchen".into(), ..option.clone() };
    update_options(&mut all, "ha".into(), "entities".into(), vec![changed.clone()]);
    assert_eq!(all[&("ha".into(), "entities".into())].as_ref(), &[changed]);
    update_options(&mut all, "ha".into(), "entities".into(), vec![]);
    assert_eq!(all.len(), 1);
    assert_eq!(all[&("other".into(), "entities".into())].as_ref(), &[option]);
}
