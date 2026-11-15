use super::*;
use crate::tests::*;

fn start(media: DataValue) -> (DownloadNode, Hub, Fake, Inputs) {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let inputs = [(PinId(1), media)].into_iter().collect();
    assert!(
        node.run(Trigger::Pin(PinId(0)), &inputs, &mut hub, &mut env)
            .is_empty()
    );
    (node, hub, env, inputs)
}

fn lookup(node: &mut DownloadNode, hub: &mut Hub, env: &mut Fake, inputs: &Inputs) {
    let id = env.requests.last().unwrap().0;
    assert!(node.run(Trigger::Task { id, result: response(200, json!({
        "ok": true, "result": {"file_size": MAX_DOWNLOAD_BYTES, "file_path": "files/one.bin"}
    })) }, inputs, hub, env).is_empty());
    let request = &env.requests.last().unwrap().1;
    assert_eq!(
        request.url,
        "https://api.telegram.org/file/botsecret/files/one.bin"
    );
    assert_eq!(request.method, "GET");
    assert!(request.save_to_blob);
}

fn file() -> DataValue {
    StructValue::new("media.document")
        .field("file_id", "telegram-file")
        .build()
}

fn assert_safe(node: &DownloadNode, out: &[Output], env: &Fake) {
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert!(out.contains(&Output::Clear(PinId(13))));
    let error = node.error.as_ref().unwrap();
    assert!(!format!("{out:?} {error:?} {:?} {:?}", env.logs, env.errors).contains("secret"));
    assert!(error.key.starts_with("telegram-"));
}

#[test]
fn download_preserves_metadata_and_type_for_all_four_media_kinds() {
    for kind in ["photo", "audio", "video", "document"] {
        let media = StructValue::new(&format!("media.{kind}"))
            .field("file_id", "file")
            .field("blob", "old")
            .field("file_name", "original.bin")
            .field("size", 200i64)
            .field("mime_type", "original/type")
            .field(
                "duration",
                DataValue::Duration(chrono::TimeDelta::seconds(3)),
            )
            .build();
        let (mut node, mut hub, mut env, inputs) = start(media.clone());
        lookup(&mut node, &mut hub, &mut env, &inputs);
        let out = node.run(
            Trigger::Task {
                id: env.requests.last().unwrap().0,
                result: Ok(TaskOutput::Blob(BlobResponse {
                    status: 200,
                    headers: vec![],
                    blob: "new".into(),
                    size: 300,
                    content_type: Some("download/type".into()),
                })),
            },
            &inputs,
            &mut hub,
            &mut env,
        );
        assert!(out.contains(&Output::Exec(PinId(10))));
        let mut expected = media;
        let DataValue::Object { fields, .. } = &mut expected else {
            unreachable!()
        };
        fields.get_mut("blob").unwrap().value = "new".into();
        assert!(out.contains(&Output::Data(PinId(13), expected)));
        assert!(env.logs.iter().all(|s| !s.contains("secret")));
    }
}

#[test]
fn download_http_and_transport_errors_never_expose_credential_url() {
    for result in [
        Err("GET https://api.telegram.org/file/botsecret/files/one.bin failed".into()),
        response(
            404,
            json!({"ok":false,"description":"https://api.telegram.org/file/botsecret/files/one.bin"}),
        ),
        Ok(TaskOutput::Delay),
    ] {
        let (mut node, mut hub, mut env, inputs) = start(file());
        lookup(&mut node, &mut hub, &mut env, &inputs);
        let out = node.run(
            Trigger::Task {
                id: env.requests.last().unwrap().0,
                result,
            },
            &inputs,
            &mut hub,
            &mut env,
        );
        assert_safe(&node, &out, &env);
    }
}

#[test]
fn download_file_request_429_obeys_retry_after_and_retries_original_request() {
    let (mut node, mut hub, mut env, inputs) = start(file());
    lookup(&mut node, &mut hub, &mut env, &inputs);
    let original = env.requests.last().unwrap().1.clone();
    for retry in 0..=5 {
        let out = node.run(
            Trigger::Task {
                id: env.requests.last().unwrap().0,
                result: response(
                    429,
                    json!({"ok":false,"description":"retry secret","parameters":{"retry_after":7}}),
                ),
            },
            &inputs,
            &mut hub,
            &mut env,
        );
        if retry == 5 {
            assert_safe(&node, &out, &env);
            assert_eq!(env.delays.len(), 5);
            break;
        }
        assert!(out.is_empty());
        assert_eq!(env.delays.last().unwrap().1, 7);
        assert!(
            node.run(
                Trigger::Task {
                    id: env.delays.last().unwrap().0,
                    result: Ok(TaskOutput::Delay),
                },
                &inputs,
                &mut hub,
                &mut env
            )
            .is_empty()
        );
        let request = &env.requests.last().unwrap().1;
        assert_eq!(request.url, original.url);
        assert_eq!(request.method, original.method);
        assert_eq!(request.body, original.body);
        assert!(request.save_to_blob);
    }
}

#[test]
fn download_rejects_invalid_media_paths_and_kind_without_wrong_output_contract() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    for media in [
        "text".into(),
        StructValue::new("media.other")
            .field("file_id", "id")
            .build(),
        StructValue::new("media.photo").build(),
    ] {
        let mut node = DownloadNode::new();
        let inputs = [(PinId(1), media)].into_iter().collect();
        let out = node.run(Trigger::Pin(PinId(0)), &inputs, &mut hub, &mut env);
        assert!(out.contains(&Output::Exec(PinId(11))));
        assert_eq!(node.error.unwrap().key, "telegram-error-media");
        assert!(env.requests.is_empty());
    }
    let mut node = DownloadNode::new();
    node.set_property("kind", "photo".into());
    let inputs = [(PinId(1), file())].into_iter().collect();
    assert!(
        node.run(Trigger::Pin(PinId(0)), &inputs, &mut hub, &mut env)
            .contains(&Output::Exec(PinId(11)))
    );
    assert_eq!(node.error.unwrap().key, "telegram-error-download-kind");
    for path in [json!(null), json!(""), json!("  ")] {
        let (mut node, mut hub, mut env, inputs) = start(file());
        let out = node.run(
            Trigger::Task {
                id: env.requests[0].0,
                result: response(200, json!({"ok": true,"result":{"file_path":path}})),
            },
            &inputs,
            &mut hub,
            &mut env,
        );
        assert!(out.contains(&Output::Exec(PinId(11))));
        assert_eq!(env.requests.len(), 1);
    }
}

#[test]
fn download_ignores_foreign_tasks_and_invalidates_work_after_settings_change() {
    for downloading in [false, true] {
        let (mut node, mut hub, mut env, inputs) = start(file());
        if downloading {
            lookup(&mut node, &mut hub, &mut env, &inputs);
        }
        let count = env.requests.len();
        assert!(
            node.run(
                Trigger::Task {
                    id: u64::MAX,
                    result: Ok(TaskOutput::Delay)
                },
                &inputs,
                &mut hub,
                &mut env
            )
            .is_empty()
        );
        assert!(node.pending.is_some());
        hub.generation += 1;
        let out = node.run(
            Trigger::Task {
                id: env.requests.last().unwrap().0,
                result: response(200, json!({"ok":true})),
            },
            &inputs,
            &mut hub,
            &mut env,
        );
        assert!(out.contains(&Output::Exec(PinId(11))));
        assert_eq!(node.error.unwrap().key, "telegram-error-changed");
        assert_eq!(env.requests.len(), count);
    }
}

#[test]
fn blob_passthrough_does_not_require_network_authentication() {
    let (mut hub, mut env) = (Hub::default(), Fake::default());
    let mut node = DownloadNode::new();
    let media = StructValue::new("media.video")
        .field("blob", "stored")
        .build();
    let inputs = [(PinId(1), media.clone())].into_iter().collect();
    assert_eq!(
        node.run(Trigger::Pin(PinId(0)), &inputs, &mut hub, &mut env),
        DownloadNode::completed(media)
    );
    assert!(env.requests.is_empty());
}
