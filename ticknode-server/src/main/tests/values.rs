use super::*;

#[tokio::test]
async fn init_is_slim_and_entry_fields_are_requested_over_websocket() {
    let Some(dir) = test_plugin_dir() else { return };
    let server = start(dir.path()).await;
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{}/ws", server.addr))
        .await
        .unwrap();
    let ServerMessage::Init { value_catalogs, .. } = recv(&mut ws).await else {
        panic!("expected Init")
    };
    let catalog = value_catalogs.iter().find(|c| c.namespace == "ha").unwrap();
    assert_eq!(catalog.entries.len(), 2);
    let entry = &catalog.entries[0];
    assert_eq!(entry.key, "sensor.one");
    assert_eq!(entry.title, "sensor.one");
    assert!(serde_json::to_value(entry).unwrap().get("fields").is_none());
    for key in ["sensor.one", "missing"] {
        send(
            &mut ws,
            ClientMessage::ValueEntryFields {
                namespace: "ha".into(),
                key: key.into(),
            },
        )
        .await;
        let ServerMessage::ValueEntryFields {
            namespace,
            key: received,
            fields,
        } = recv(&mut ws).await
        else {
            panic!("expected ValueEntryFields")
        };
        assert_eq!(namespace, "ha");
        assert_eq!(received, key);
        if key == "sensor.one" {
            assert_eq!(
                fields,
                vec![ValueField {
                    path: "state".into(),
                    title: "State".into(),
                    data_type: DataType::Named("quantity.power".into())
                }]
            );
        } else {
            assert!(fields.is_empty());
        }
    }
}
