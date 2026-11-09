use super::*;

#[test]
fn core_c_plugin_names_reject_empty_and_unsafe_names_but_allow_hyphens() {
    let engine = TickEngine::new();
    for name in ["", "with.dot", "../escape", "has space", "nonascii-é"] {
        let error = engine.validate_plugin_name(&PluginManifest::new(name, "1", "test")).unwrap_err();
        assert_eq!(error.fallback, format!("Invalid plugin name: {name:?}"));
    }
    for name in ["plain", "with-hyphen", "with_underscore", "123"] {
        assert_eq!(engine.validate_plugin_name(&PluginManifest::new(name, "1", "test")), Ok(()));
    }
}
