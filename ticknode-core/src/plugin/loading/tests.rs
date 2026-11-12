use super::*;

#[test]
fn core_c_plugin_names_reject_empty_and_unsafe_names_but_allow_hyphens() {
    let engine = TickEngine::new();
    for name in ["", "with.dot", "../escape", "has space", "nonascii-é"] {
        let error = engine
            .validate_plugin_name(&PluginManifest::new(name, "1", "test"))
            .unwrap_err();
        assert_eq!(error.fallback, format!("Invalid plugin name: {name:?}"));
    }
    for name in ["plain", "with-hyphen", "with_underscore", "123"] {
        assert_eq!(
            engine.validate_plugin_name(&PluginManifest::new(name, "1", "test")),
            Ok(())
        );
    }
}

#[test]
fn validates_and_filters_resource_collections_and_actions() {
    let node_types = vec!["test/valid".to_string()];
    let resources = vec![
        // Valid
        ResourceCollection::new("c1", "Collection 1", "source1").action(ResourceAction::new(
            "A1",
            "test/valid",
            "prop1",
        )),
        // Empty id
        ResourceCollection::new("", "Empty ID", "source1").action(ResourceAction::new(
            "A",
            "test/valid",
            "prop1",
        )),
        // Duplicate id
        ResourceCollection::new("c1", "Duplicate ID", "source1").action(ResourceAction::new(
            "A",
            "test/valid",
            "prop1",
        )),
        // Empty source
        ResourceCollection::new("c2", "Empty Source", "").action(ResourceAction::new(
            "A",
            "test/valid",
            "prop1",
        )),
        // Actions dropped (empty property, unknown type), leaving empty collection
        ResourceCollection::new("c3", "Invalid Actions", "source1")
            .action(ResourceAction::new("Empty Prop", "test/valid", ""))
            .action(ResourceAction::new("Unknown Type", "test/unknown", "prop1")),
        // Collection with some valid and some invalid actions
        ResourceCollection::new("c4", "Mixed Actions", "source1")
            .action(ResourceAction::new("Invalid", "test/unknown", "prop1"))
            .action(ResourceAction::new("Valid", "test/valid", "prop1")),
    ];

    let validated = validate_resources("test_plugin", resources, &node_types);
    assert_eq!(validated.len(), 2);
    assert_eq!(validated[0].id, "c1");
    assert_eq!(validated[0].actions.len(), 1);
    assert_eq!(validated[1].id, "c4");
    assert_eq!(validated[1].actions.len(), 1);
    assert_eq!(validated[1].actions[0].title, "Valid");
}
