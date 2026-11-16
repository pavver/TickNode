use super::*;

#[test]
fn slim_init_and_on_demand_fields_round_trip_without_sheet_or_history() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/wasm32-wasip2/debug/test_plugin.wasm");
    let Ok(bytes) = std::fs::read(&path) else {
        assert!(std::env::var_os("TICKNODE_REQUIRE_PLUGINS").is_none());
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut actor = test_actor(dir.path());
    actor.engine.load_plugin(&bytes, None, |_| vec![]).unwrap();
    let snapshot = actor.snapshot(None);
    let encoded = postcard::to_allocvec(&snapshot).unwrap();
    let decoded: ServerMessage = postcard::from_bytes(&encoded).unwrap();
    assert_eq!(decoded, snapshot);
    let ServerMessage::Init { value_catalogs, .. } = decoded else {
        panic!("expected Init")
    };
    assert_eq!(value_catalogs[0].entries.len(), 2);
    let entry = &value_catalogs[0].entries[0];
    assert_eq!(entry.key, "sensor.one");
    assert!(serde_json::to_value(entry).unwrap().get("fields").is_none());
    let (reply, mut rx) = mpsc::unbounded_channel();
    for (namespace, key) in [
        ("ha", "sensor.one"),
        ("ha", "absent"),
        ("absent", "sensor.one"),
    ] {
        let command = ClientMessage::ValueEntryFields {
            namespace: namespace.into(),
            key: key.into(),
        };
        let bytes = postcard::to_allocvec(&command).unwrap();
        let (events, created) = actor
            .apply(postcard::from_bytes(&bytes).unwrap(), None, &reply)
            .unwrap();
        assert!(events.is_empty());
        assert!(created.is_none());
        let response = rx.try_recv().unwrap();
        let bytes = postcard::to_allocvec(&response).unwrap();
        assert_eq!(
            postcard::from_bytes::<ServerMessage>(&bytes).unwrap(),
            response
        );
        let ServerMessage::ValueEntryFields {
            namespace: received_namespace,
            key: received_key,
            fields,
        } = response
        else {
            panic!("expected fields")
        };
        assert_eq!(
            (received_namespace.as_str(), received_key.as_str()),
            (namespace, key)
        );
        if namespace == "ha" && key == "sensor.one" {
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
    assert!(actor.dirty_sheets.is_empty());
    assert!(actor.histories.is_empty());
    let events = test_apply(
        &mut actor,
        ClientMessage::Batch(vec![ClientMessage::ValueEntryFields {
            namespace: "ha".into(),
            key: "sensor.one".into(),
        }]),
    );
    assert!(events.is_empty());
}
