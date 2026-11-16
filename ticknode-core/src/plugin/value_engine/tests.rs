use super::*;
use std::collections::HashMap;
fn engine() -> Option<TickEngine> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            assert!(
                std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(),
                "{}: {error}",
                path.display()
            );
            return None;
        }
    };
    let mut engine = TickEngine::new();
    engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    Some(engine)
}
#[test]
fn publications_invalidate_pure_caches_immediately_and_do_not_touch_active_outputs() {
    let Some(mut engine) = engine() else { return };
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "text/template",
            &[("template".into(), "{{ ha.sensor.one.state }}".into())],
        )
        .unwrap();
    engine.tick();
    engine
        .pure_cache
        .insert(node, HashMap::from([(PinId(0), "cached".into())]));
    engine
        .data_cache
        .insert(node, HashMap::from([(PinId(0), "active".into())]));
    let value = DataValue::Object {
        type_name: None,
        fields: [(
            "state".into(),
            ObjectField {
                data_type: None,
                value: 42i64.into(),
            },
        )]
        .into(),
    };
    let values = lock(&engine.plugins["test_plugin"].runtime).values.clone();
    lock(&values)
        .pending
        .insert("sensor.one".into(), Some(value));
    engine.sync_plugin_values();
    assert!(engine.pure_cache.is_empty());
    assert_eq!(
        engine.data_cache[&node][&PinId(0)],
        DataValue::from("active")
    );
    let result = engine.pull_data(node, PinId(0), &mut Vec::new()).unwrap();
    assert_eq!(result, DataValue::from("42"));
    lock(&values).pending.insert("sensor.one".into(), None);
    engine.sync_plugin_values();
    assert!(engine.pure_cache.is_empty());
    assert!(
        engine
            .plugin_values
            .resolve("ha.sensor.one.state")
            .is_none()
    );
}
#[test]
fn namespace_validation_rejects_reserved_roots_and_loaded_owners() {
    let Some(engine) = engine() else { return };
    for root in [
        "var", "sheet", "global", "text", "loop", "range", "dict", "now", "True", "False", "None",
        "caller", "ha", "bad.path", "",
    ] {
        let mut manifest = PluginManifest::new("another", "1", "test").values_namespace(root);
        engine.validate_values_namespace(&mut manifest);
        assert!(manifest.values_namespace.is_none(), "{root}");
    }
    let mut manifest = PluginManifest::new("another", "1", "test").values_namespace("other");
    engine.validate_values_namespace(&mut manifest);
    assert_eq!(manifest.values_namespace.as_deref(), Some("other"));
}

#[test]
fn restoring_the_same_graph_before_the_watch_batch_keeps_unchanged_values() {
    let Some(mut engine) = engine() else { return };
    let node = NodeId::generate();
    engine
        .create_node(
            node,
            "text/template",
            &[("template".into(), "{{ ha.sensor.one.state }}".into())],
        )
        .unwrap();
    engine.tick();
    let graph = engine.export_graph();
    let value = engine
        .plugin_values
        .resolve("ha.sensor.one.state")
        .unwrap()
        .0
        .clone();
    engine.load_graph(&graph);
    assert_eq!(
        engine
            .plugin_values
            .resolve("ha.sensor.one.state")
            .unwrap()
            .0,
        &value
    );
}

#[test]
fn input_bindings_convert_quantity_fields_and_wires_release_the_watch() {
    let Some(mut engine) = engine() else { return };
    let consumer = NodeId::generate();
    engine.create_node(consumer, "media/fetch", &[]).unwrap();
    engine
        .set_input_variable(
            consumer,
            PinId(1),
            VariableRef::parse("ha.sensor.one.state"),
        )
        .unwrap();
    engine.tick();
    let inputs = engine.pull_inputs(consumer, &mut Vec::new());
    assert_eq!(inputs[&PinId(1)], Some(DataValue::from("1 kW")));
    let source = NodeId::generate();
    engine
        .create_node(source, "const", &[("value".into(), "literal".into())])
        .unwrap();
    engine
        .connect_data(source, PinId(0), consumer, PinId(1))
        .unwrap();
    assert!(!engine.plugin_values.is_watched("ha.sensor.one"));
    engine.tick();
    assert_eq!(
        engine.pull_inputs(consumer, &mut Vec::new())[&PinId(1)],
        Some(DataValue::from("literal"))
    );
    engine.remove_node(source);
    assert!(engine.plugin_values.is_watched("ha.sensor.one"));
    engine.tick();
    engine
        .set_input_variable(
            consumer,
            PinId(1),
            VariableRef::parse("ha.sensor.one.absent"),
        )
        .unwrap();
    engine.host_state.logs.clear();
    for _ in 0..2 {
        assert_eq!(
            engine.pull_inputs(consumer, &mut Vec::new())[&PinId(1)],
            None
        );
    }
    assert_eq!(
        engine
            .host_state
            .logs
            .iter()
            .filter(|(node, line)| *node == consumer && line.contains("ha.sensor.one.absent"))
            .count(),
        1
    );
}

#[test]
fn catalog_changes_are_diffed_and_missing_formula_fields_report_translatable_errors() {
    let Some(mut engine) = engine() else { return };
    let changes = engine.take_value_catalog_changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].entries.len(), 2);
    assert!(engine.take_value_catalog_changes().is_empty());
    engine
        .configure_plugin("test_plugin", &[("token".into(), "ok".into())])
        .unwrap();
    assert!(engine.take_value_catalog_changes().is_empty());
    let formula = NodeId::generate();
    engine
        .create_node(
            formula,
            "math/expression",
            &[("expression".into(), "ha.sensor.one.absent + 1".into())],
        )
        .unwrap();
    engine.tick();
    assert!(
        engine
            .pull_data(formula, PinId(0), &mut Vec::new())
            .is_none()
    );
    assert!(
        engine
            .host_state
            .logs
            .iter()
            .any(|(node, line)| *node == formula && line.contains("ha.sensor.one.absent"))
    );
    engine.disable_plugin("test_plugin").unwrap();
    let changes = engine.take_value_catalog_changes();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].entries.is_empty());
}
