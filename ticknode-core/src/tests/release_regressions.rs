use super::*;

#[test]
fn absent_pure_results_are_cached_until_the_next_tick() {
    let mut engine = TickEngine::new();
    let node = NodeId::generate();
    engine.create_node(node, "variable/get", &[]).unwrap();
    for _ in 0..3 {
        assert_eq!(engine.pull_data(node, PinId(0), &mut vec![]), None);
    }
    assert_eq!(
        engine.last_tick.spent, 1,
        "missing results are still results"
    );
    engine.tick();
    assert_eq!(engine.pull_data(node, PinId(0), &mut vec![]), None);
    assert_eq!(engine.executed.len(), 1);
}

#[test]
fn media_kind_validation_rejects_invalid_edits_without_changing_the_graph() {
    let mut engine = TickEngine::new();
    let node = NodeId::generate();
    engine.create_node(node, "media/fetch", &[]).unwrap();
    for invalid in [DataValue::from("exe"), 5i64.into()] {
        let before = engine.export_graph();
        assert!(engine.set_node_property(node, "kind", invalid).is_err());
        assert_eq!(engine.export_graph(), before);
    }
    for kind in ["audio", "video", "document", "photo"] {
        engine.set_node_property(node, "kind", kind.into()).unwrap();
        assert!(
            engine
                .node_definition(node)
                .unwrap()
                .pins
                .iter()
                .any(|pin| {
                    pin.id == PinId(13)
                        && pin.pin_type == PinType::Data(DataType::Named(format!("media.{kind}")))
                })
        );
    }
}

#[test]
fn registry_ownership_lists_and_removes_only_the_selected_plugin() {
    let mut registry = NodeRegistry::new();
    registry.register_owned("first", Box::new(|id| Box::new(crate::ConsoleNode { id })));
    registry.register_owned(
        "second",
        Box::new(|id| {
            Box::new(crate::ClickButtonNode {
                id,
                label: String::new(),
            })
        }),
    );
    assert_eq!(registry.types_owned_by("first"), ["console"]);
    assert_eq!(registry.unregister_owner("first"), ["console"]);
    assert!(
        registry
            .create_node("console", NodeId::generate())
            .is_none()
    );
    assert!(
        registry
            .create_node("test/click_button", NodeId::generate())
            .is_some()
    );
    assert!(registry.create_node("clock", NodeId::generate()).is_some());
    assert_eq!(registry.owner_of("test/click_button"), Some("second"));
}

#[test]
fn builtin_editor_contracts_keep_adaptive_types_explicit() {
    let registry = NodeRegistry::new();
    for name in [
        "clock",
        "sleep",
        "media/fetch",
        "object/make",
        "test/click_button",
        "console",
        "match",
    ] {
        let def = registry.get_definition(name).unwrap();
        assert!(
            !def.adaptive,
            "{name} has no implicit connection adaptation"
        );
    }
    assert!(registry.get_definition("variable/get").unwrap().adaptive);
    assert!(registry.get_definition("object/break").unwrap().adaptive);
    let media = registry.get_definition("media/fetch").unwrap();
    assert!(media.pins.iter().any(|p| p.id == PinId(0)
        && p.pin_type == PinType::Exec
        && p.direction == PinDirection::Input));
    assert!(media.pins.iter().any(|p| p.id == PinId(1) && p.required));
    let variable = registry.get_definition("variable/get").unwrap();
    assert!(
        variable
            .pins
            .iter()
            .any(|p| p.id == PinId(1) && p.pin_type == PinType::Data(DataType::Bool))
    );
}
