use super::*;

/// Options published by a plugin reach editors in `Init`, changes are broadcast once, and disabling the
/// plugin sends the sources again as empty lists.
#[tokio::test]
async fn property_options_are_sent_in_init_and_broadcast_on_change() {
    use ticknode_shared::{PropertyOption, SettingUpdate};
    let Some(dir) = test_plugin_dir() else { return };
    let server = start(dir.path()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    let mut seen = Vec::new();

    // Configure (empty token) still publishes the default greeting
    let ServerMessage::Init {
        property_options,
        definitions,
        ..
    } = recv_logged(&mut ws, &mut seen).await
    else {
        panic!("expected Init")
    };
    let greetings = property_options
        .iter()
        .find(|s| s.plugin == "test_plugin" && s.source == "greetings")
        .expect("Init carries the options");
    assert_eq!(
        greetings
            .options
            .iter()
            .map(|o| o.value.as_str())
            .collect::<Vec<_>>(),
        ["Hello"]
    );
    assert!(
        definitions
            .iter()
            .any(|d| d.type_name == "test/picker" && !d.properties.is_empty())
    );

    // Changing the setting republishes the source
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![(
                "greeting".into(),
                SettingUpdate::Set(DataValue::String("Hola".into())),
            )],
        },
    )
    .await;
    let mut got = None;
    for _ in 0..500 {
        // The initial list may be re-broadcast after Init (events can repeat): wait for the new one.
        if let ServerMessage::PropertyOptions {
            plugin,
            source,
            options,
        } = recv_any(&mut ws).await
            && source == "greetings"
            && options.first().is_some_and(|o| o.value == "Hola")
        {
            got = Some((plugin, source, options));
            break;
        }
    }
    let (plugin, source, options) = got.expect("a PropertyOptions broadcast");
    assert_eq!(
        (plugin.as_str(), source.as_str()),
        ("test_plugin", "greetings")
    );
    assert_eq!(
        options,
        vec![PropertyOption {
            value: "Hola".into(),
            label: "HOLA".into(),
            detail: "detail Hola".into(),
            icon: Some("mdi:hand-wave".into())
        }]
    );

    // The same value again does not broadcast anything (only PluginUpdated follows)
    send(
        &mut ws,
        ClientMessage::ConfigurePlugin {
            name: "test_plugin".into(),
            values: vec![(
                "greeting".into(),
                SettingUpdate::Set(DataValue::String("Hola".into())),
            )],
        },
    )
    .await;
    loop {
        match recv_any(&mut ws).await {
            ServerMessage::PropertyOptions { source, .. } if source == "greetings" => {
                panic!("an unchanged list was broadcast")
            }
            ServerMessage::PluginUpdated(_) => break,
            _ => {}
        }
    }
    // Give the engine a few ticks to prove nothing follows
    tokio::time::sleep(Duration::from_millis(100)).await;
    while let Ok(Some(Ok(Message::Binary(b)))) =
        tokio::time::timeout(Duration::from_millis(50), ws.next()).await
    {
        assert!(
            !matches!(
                postcard::from_bytes::<ServerMessage>(&b).unwrap(),
                ServerMessage::PropertyOptions { .. }
            ),
            "an unchanged list was broadcast"
        );
    }

    // Disabling removes the sources: they arrive as empty lists
    send(
        &mut ws,
        ClientMessage::SetPluginEnabled {
            name: "test_plugin".into(),
            enabled: false,
        },
    )
    .await;
    let mut emptied = Vec::new();
    for _ in 0..40 {
        match recv_any(&mut ws).await {
            // The options follow the plugin events of the same command
            ServerMessage::PropertyOptions {
                source, options, ..
            } => {
                emptied.push((source, options.len()));
                break;
            }
            _ => {}
        }
    }
    assert!(
        emptied.contains(&("greetings".to_string(), 0)),
        "{emptied:?}"
    );

    // A new editor gets nothing for a disabled plugin
    let (mut ws2, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    let ServerMessage::Init {
        property_options, ..
    } = recv_logged(&mut ws2, &mut seen).await
    else {
        panic!("expected Init")
    };
    assert!(property_options.is_empty());
    server.handle.shutdown().await;
}
