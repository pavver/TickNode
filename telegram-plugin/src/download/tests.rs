use super::*;
use crate::tests::*;
use serde_json::json;

fn inputs(values: &[(u32, DataValue)]) -> Inputs {
    values
        .iter()
        .map(|(id, v)| (PinId(*id), v.clone()))
        .collect()
}

fn sample_photo_file_id(file_id: &str) -> DataValue {
    StructValue::new("media.photo")
        .field("file_id", file_id)
        .field("width", 1280i64)
        .field("height", 720i64)
        .build()
}

#[test]
fn download_get_file_then_save_to_blob_request_and_output_preservation() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = sample_photo_file_id("tg_photo_100");
    let values = inputs(&[(1, media)]);

    // 1. Trigger pin In -> starts getFile request
    let out = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    assert!(out.is_empty());
    assert_eq!(env.requests.len(), 1);
    let (get_file_task, get_file_req) = &env.requests[0];
    assert!(get_file_req.url.contains("/getFile"));
    assert_eq!(
        get_file_req.body,
        json!({"file_id": "tg_photo_100"}).to_string().into_bytes()
    );
    let get_file_task = *get_file_task;

    // 2. getFile returns ok with file_path
    let out = node.run(
        Trigger::Task {
            id: get_file_task,
            result: response(
                200,
                json!({
                    "ok": true,
                    "result": {
                        "file_id": "tg_photo_100",
                        "file_unique_id": "uniq_100",
                        "file_size": 45000,
                        "file_path": "photos/file_100.jpg"
                    }
                }),
            ),
        },
        &values,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty());
    assert_eq!(env.requests.len(), 2);
    let (download_task, download_req) = &env.requests[1];
    assert!(
        download_req
            .url
            .ends_with("/file/botsecret/photos/file_100.jpg")
    );
    assert!(download_req.save_to_blob);
    let download_task = *download_task;

    // 3. Blob response finishes
    let out = node.run(
        Trigger::Task {
            id: download_task,
            result: Ok(TaskOutput::Blob(BlobResponse {
                status: 200,
                headers: vec![],
                blob: "blob_saved_100".into(),
                size: 45000,
                content_type: Some("image/jpeg".into()),
            })),
        },
        &values,
        &mut hub,
        &mut env,
    );

    // Outputs: Done (10), Clear Error (12), Media (13)
    assert!(out.contains(&Output::Exec(PinId(10))));
    assert!(out.contains(&Output::Clear(PinId(12))));

    let media_out = out
        .iter()
        .find_map(|o| match o {
            Output::Data(PinId(13), val) => Some(val),
            _ => None,
        })
        .expect("Media data output");

    let DataValue::Object { type_name, fields } = media_out else {
        panic!("expected Object");
    };
    assert_eq!(type_name.as_deref(), Some("media.photo"));
    // Kept input fields
    assert_eq!(
        fields["file_id"].value,
        DataValue::String("tg_photo_100".into())
    );
    assert_eq!(
        fields["width"].value,
        DataValue::Number(NumberValue::Int(1280))
    );
    assert_eq!(
        fields["height"].value,
        DataValue::Number(NumberValue::Int(720))
    );
    // Added fields
    assert_eq!(
        fields["blob"].value,
        DataValue::String("blob_saved_100".into())
    );
    assert_eq!(
        fields["size"].value,
        DataValue::Number(NumberValue::Int(45000))
    );
    assert_eq!(
        fields["mime_type"].value,
        DataValue::String("image/jpeg".into())
    );
}

#[test]
fn download_exceeds_20mb_error() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = sample_photo_file_id("tg_big_file");
    let values = inputs(&[(1, media)]);

    let _ = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    let task_id = env.requests[0].0;

    let out = node.run(
        Trigger::Task {
            id: task_id,
            result: response(
                200,
                json!({
                    "ok": true,
                    "result": {
                        "file_id": "tg_big_file",
                        "file_size": 20 * 1024 * 1024 + 1,
                        "file_path": "photos/big.jpg"
                    }
                }),
            ),
        },
        &values,
        &mut hub,
        &mut env,
    );

    // Fails without starting download
    assert_eq!(env.requests.len(), 1);
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(
        node.error.as_ref().map(|e| e.key.as_str()),
        Some("telegram-error-download-size")
    );
}

#[test]
fn download_url_only_error() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = StructValue::new("media.photo")
        .field("url", "https://example.com/cam.jpg")
        .build();
    let values = inputs(&[(1, media)]);

    let out = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    assert_eq!(env.requests.len(), 0);
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(
        node.error.as_ref().map(|e| e.key.as_str()),
        Some("telegram-error-download-url")
    );
    assert_eq!(
        node.error.as_ref().map(|e| e.fallback.as_str()),
        Some("Only Telegram files can be downloaded; use Download media (media/fetch) for URLs")
    );
}

#[test]
fn download_blob_passthrough() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = StructValue::new("media.photo")
        .field("blob", "existing_blob_id")
        .field("size", 12345i64)
        .build();
    let values = inputs(&[(1, media.clone())]);

    let out = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    assert_eq!(env.requests.len(), 0);
    assert!(out.contains(&Output::Exec(PinId(10))));
    assert!(out.contains(&Output::Clear(PinId(12))));
    assert!(out.contains(&Output::Data(PinId(13), media)));
}

#[test]
fn download_token_never_in_outputs_or_errors() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = sample_photo_file_id("photo_token_test");
    let values = inputs(&[(1, media)]);

    // 1. Error on getFile containing token
    let _ = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    let task_id = env.requests[0].0;
    let out = node.run(
        Trigger::Task {
            id: task_id,
            result: response(
                400,
                json!({
                    "ok": false,
                    "error_code": 400,
                    "description": "Bad Request on token secret at path"
                }),
            ),
        },
        &values,
        &mut hub,
        &mut env,
    );

    assert!(out.contains(&Output::Exec(PinId(11))));
    let err_str = out
        .iter()
        .find_map(|o| match o {
            Output::Data(PinId(12), DataValue::String(s)) => Some(s),
            _ => None,
        })
        .expect("Error pin string");
    assert!(!err_str.contains("secret"));
    assert!(err_str.contains("[redacted]"));
    assert!(!node.error.as_ref().unwrap().fallback.contains("secret"));
}

#[test]
fn download_429_rate_limit_retry() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    let mut node = DownloadNode::new();
    let media = sample_photo_file_id("photo_retry_test");
    let values = inputs(&[(1, media)]);

    let _ = node.run(Trigger::Pin(PinId(0)), &values, &mut hub, &mut env);
    let task_id = env.requests[0].0;

    // getFile returns 429
    let out = node.run(
        Trigger::Task {
            id: task_id,
            result: response(
                429,
                json!({
                    "ok": false,
                    "error_code": 429,
                    "description": "Too many requests",
                    "parameters": {"retry_after": 3}
                }),
            ),
        },
        &values,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty());
    assert_eq!(env.delays.len(), 1);
    assert_eq!(env.delays[0].1, 3);

    // Complete delay task -> retries getFile
    let delay_id = env.delays[0].0;
    let out = node.run(
        Trigger::Task {
            id: delay_id,
            result: Ok(TaskOutput::Delay),
        },
        &values,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty());
    assert_eq!(env.requests.len(), 2);
    assert!(env.requests[1].1.url.contains("/getFile"));
}

#[test]
fn download_kind_property_changes_output_pin_type() {
    let mut node = DownloadNode::new();
    // Default auto -> Any
    let def = node.definition();
    let media_pin = def.pins.iter().find(|p| p.id == PinId(13)).unwrap();
    assert_eq!(media_pin.pin_type, PinType::Data(DataType::Any));

    // Photo -> Named("media.photo")
    node.set_property("kind", "photo".into());
    let def = node.definition();
    let media_pin = def.pins.iter().find(|p| p.id == PinId(13)).unwrap();
    assert_eq!(
        media_pin.pin_type,
        PinType::Data(DataType::Named("media.photo".into()))
    );

    // Document -> Named("media.document")
    node.set_property("kind", "document".into());
    let def = node.definition();
    let media_pin = def.pins.iter().find(|p| p.id == PinId(13)).unwrap();
    assert_eq!(
        media_pin.pin_type,
        PinType::Data(DataType::Named("media.document".into()))
    );

    // Reset to auto -> Any
    node.set_property("kind", "auto".into());
    let def = node.definition();
    let media_pin = def.pins.iter().find(|p| p.id == PinId(13)).unwrap();
    assert_eq!(media_pin.pin_type, PinType::Data(DataType::Any));
}
