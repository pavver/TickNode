use super::*;

#[tokio::test]
async fn plugins_are_managed_over_websocket() {
    use ticknode_shared::{PluginState, SettingUpdate, SettingValueView};
    let Some(dir) = test_plugin_dir() else { return };
    let server = start(dir.path()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    let mut seen = Vec::new();

    // Init: plugin is enabled, but configure rejected the empty token
    let ServerMessage::Init { sheets, .. } = recv_logged(&mut ws, &mut seen).await else {
        panic!("expected Init")
    };
    let sheet_id = sheets[0].info.id;
    send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
    let ServerMessage::Init {
        plugins,
        definitions,
        translations,
        ..
    } = recv_logged(&mut ws, &mut seen).await
    else {
        panic!("expected Init")
    };
    let info = plugin_of(&plugins);
    assert_eq!(
        (info.state, info.file.as_str()),
        (PluginState::Enabled, "test_plugin.wasm")
    );
    assert_eq!(
        info.settings_error.as_ref().map(|e| e.key.as_str()),
        Some("test-plugin-error-no-token")
    );
    // Core and plugin translations arrive with Init
    assert!(translations.iter().any(|(l, t)| l == "uk"
        && t.contains("node-const-title")
        && t.contains("node-test-greeter-description")));
    assert_eq!(info.settings.len(), 8);
    assert!(info.values.contains(&(
        "token".to_string(),
        SettingValueView::Secret { is_set: false }
    )));
    assert!(info.values.contains(&(
        "greeting".to_string(),
        SettingValueView::Value(DataValue::String("Hello".into()))
    )));
    assert!(definitions.iter().any(|d| d.type_name == "test/greeter"));
    assert!(
        !dir.path().join("plugin-config/test_plugin.json").exists(),
        "no config file without user actions"
    );

    // Save settings: secret in the response only indicates that it is set
    let secret = "s3cret-token-value";
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![
                (
                    "token".into(),
                    SettingUpdate::Set(DataValue::String(secret.into())),
                ),
                (
                    "greeting".into(),
                    SettingUpdate::Set(DataValue::String("Hello there".into())),
                ),
            ],
        },
    )
    .await;
    let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else {
        panic!("expected PluginUpdated")
    };
    assert_eq!(info.settings_error, None);
    assert!(info.values.contains(&(
        "token".to_string(),
        SettingValueView::Secret { is_set: true }
    )));
    assert!(info.values.contains(&(
        "greeting".to_string(),
        SettingValueView::Value(DataValue::String("Hello there".into()))
    )));

    // Config file: token is saved, permissions are 0600
    let config_path = dir.path().join("plugin-config/test_plugin.json");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(saved["enabled"], true);
    assert!(saved["settings"]["token"].to_string().contains(secret));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&config_path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    // Plugin node sees the updated settings without a restart
    send(
        &mut ws,
        ClientMessage::AddNode {
            type_name: "test/greeter".into(),
            position: Position { x: 3.0, y: 4.0 },
        },
    )
    .await;
    let ServerMessage::NodeAdded(greeter) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(greeter.unavailable, None);
    assert!(matches!(
        recv_logged(&mut ws, &mut seen).await,
        ServerMessage::Created(_)
    ));
    send(
        &mut ws,
        ClientMessage::TriggerNode {
            id: greeter.model.id,
            pin: PinId(0),
        },
    )
    .await;
    loop {
        if let ServerMessage::Executed(traces) = recv_any(&mut ws).await {
            if let Some(t) = traces.iter().find(|t| t.node_id == greeter.model.id) {
                assert_eq!(
                    t.outputs,
                    vec![(
                        PinId(1),
                        DataValue::String("Hello there|quiet|token".into())
                    )]
                );
                break;
            }
        }
    }

    // configure error: plugin stays loaded, error is reported in PluginUpdated
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![(
                "token".into(),
                SettingUpdate::Set(DataValue::String("bad".into())),
            )],
        },
    )
    .await;
    let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(
        (
            info.state,
            info.settings_error.as_ref().map(|e| e.fallback.as_str())
        ),
        (PluginState::Enabled, Some("Invalid token"))
    );
    // `Keep` preserves the secret; validation error goes only to the initiator
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![
                (
                    "token".into(),
                    SettingUpdate::Set(DataValue::String(secret.into())),
                ),
                (
                    "greeting".into(),
                    SettingUpdate::Set(DataValue::Number(ticknode_shared::NumberValue::Int(1))),
                ),
            ],
        },
    )
    .await;
    let ServerMessage::Error(e) = recv_logged(&mut ws, &mut seen).await else {
        panic!("expected Error")
    };
    assert_eq!(e.key, "error-setting-invalid");
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![("token".into(), SettingUpdate::Keep)],
        },
    )
    .await;
    let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(
        info.settings_error.as_ref().map(|e| e.fallback.as_str()),
        Some("Invalid token"),
        "Keep does not change the saved token"
    );
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "nope".into(),
            values: vec![],
        },
    )
    .await;
    let ServerMessage::Error(e) = recv_logged(&mut ws, &mut seen).await else {
        panic!("expected Error")
    };
    assert_eq!(
        (e.key.as_str(), e.args.clone()),
        (
            "error-plugin-not-found",
            vec![("plugin".to_string(), "nope".to_string())]
        )
    );

    // Disabling: catalog drops plugin types, node becomes a stub, followed by PluginUpdated
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![(
                "token".into(),
                SettingUpdate::Set(DataValue::String(secret.into())),
            )],
        },
    )
    .await;
    assert!(matches!(
        recv_logged(&mut ws, &mut seen).await,
        ServerMessage::PluginUpdated(_)
    ));
    send(
        &mut ws,
        ClientMessage::SetPluginEnabled {
            name: "test_plugin".into(),
            enabled: false,
        },
    )
    .await;
    let ServerMessage::Catalog {
        definitions,
        types,
        translations,
    } = recv_logged(&mut ws, &mut seen).await
    else {
        panic!("expected Catalog")
    };
    assert!(
        !definitions
            .iter()
            .any(|d| d.type_name == "test/greeter" || d.type_name.starts_with("type/make/"))
            && types.iter().all(|t| t.name.starts_with("quantity."))
    );
    assert!(
        !translations
            .iter()
            .any(|(_, t)| t.contains("node-test-greeter")),
        "the disabled plugin takes its translations away"
    );
    let ServerMessage::NodeUpdated(stub) = recv_logged(&mut ws, &mut seen).await else {
        panic!("expected NodeUpdated")
    };
    let unavailable = stub.unavailable.as_ref().expect("stub");
    assert_eq!(
        (unavailable.key.as_str(), unavailable.fallback.as_str()),
        (
            "error-node-plugin-unavailable",
            "plugin test_plugin unavailable"
        )
    );
    assert_eq!(stub.model.type_name, "test/greeter");
    assert_eq!(stub.definition.pins.len(), 2);
    let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(info.state, PluginState::Disabled);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
    assert_eq!(saved["enabled"], false);

    // Secret never appeared in any server messages
    assert!(
        !contains(&seen, secret.as_bytes()),
        "the secret leaked into messages to the editor"
    );

    // Restart: plugin stays disabled, graph preserves the plugin node as a stub
    server.handle.shutdown().await;
    let sheet_file = dir.path().join("sheets").join(format!("{}.json", sheet_id));
    let on_disk: ticknode_shared::GraphModel =
        serde_json::from_str(&std::fs::read_to_string(sheet_file).unwrap()).unwrap();
    assert_eq!(on_disk.nodes.len(), 1);
    assert_eq!(on_disk.nodes[0].position, Position { x: 3.0, y: 4.0 });
    let server = start(dir.path()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    let ServerMessage::Init { .. } = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    send(&mut ws, ClientMessage::OpenSheet(Some(sheet_id))).await;
    let ServerMessage::Init { plugins, nodes, .. } = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(plugin_of(&plugins).state, PluginState::Disabled);
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].unavailable.is_some());

    // Enabling on the fly revives the node with the same settings
    send(
        &mut ws,
        ClientMessage::SetPluginEnabled {
            name: "test_plugin".into(),
            enabled: true,
        },
    )
    .await;
    assert!(matches!(
        recv_logged(&mut ws, &mut seen).await,
        ServerMessage::Catalog { .. }
    ));
    let ServerMessage::NodeUpdated(live) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(live.unavailable, None);
    let ServerMessage::PluginUpdated(info) = recv_logged(&mut ws, &mut seen).await else {
        panic!()
    };
    assert_eq!(
        (info.state, info.settings_error),
        (PluginState::Enabled, None)
    );
    assert!(!contains(&seen, secret.as_bytes()));
    server.handle.shutdown().await;
}
