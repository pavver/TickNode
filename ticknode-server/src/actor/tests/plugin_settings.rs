use super::*;

#[test]
fn asynchronous_plugin_settings_errors_are_broadcast_without_graph_nodes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(&path) else {
        assert!(
            std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none(),
            "{} not found",
            path.display()
        );
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("plugins")).unwrap();
    std::fs::write(dir.path().join("plugins/test_plugin.wasm"), bytes).unwrap();
    let mut actor = test_actor(dir.path());
    actor.plugins.load_all(&mut actor.engine);
    actor
        .engine
        .configure_plugin(
            "test_plugin",
            &[
                ("token".into(), "good".into()),
                ("greeting".into(), "settings-error".into()),
            ],
        )
        .unwrap();
    let mut receiver = actor.events.subscribe();
    actor.tick();
    let mut updates = Vec::new();
    while let Ok((tag, bytes)) = receiver.try_recv() {
        if let ServerMessage::PluginUpdated(info) = postcard::from_bytes(&bytes).unwrap() {
            assert_eq!(tag, None);
            updates.push(info);
        }
    }
    assert_eq!(updates.len(), 1);
    assert_eq!(
        updates[0].settings_error.as_ref().unwrap().key,
        "test-plugin-error-async"
    );
    assert!(actor.engine.node_errors().is_empty());
    actor.tick();
    while let Ok((_, bytes)) = receiver.try_recv() {
        assert!(!matches!(
            postcard::from_bytes::<ServerMessage>(&bytes).unwrap(),
            ServerMessage::PluginUpdated(_)
        ));
    }
}
