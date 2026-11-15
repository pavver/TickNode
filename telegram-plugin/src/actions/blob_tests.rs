use super::tests::*;
use super::*;
use crate::tests::*;
use serde_json::json;
use ticknode_plugin_sdk::*;

fn blob_media_val(
    type_name: &str,
    file_id: Option<&str>,
    blob: Option<&str>,
    url: Option<&str>,
) -> DataValue {
    let mut s = StructValue::new(type_name);
    if let Some(fid) = file_id {
        s = s.field("file_id", fid);
    }
    if let Some(b) = blob {
        s = s.field("blob", b);
    }
    if let Some(u) = url {
        s = s.field("url", u);
    }
    s.build()
}

#[test]
fn send_bare_photo_with_blob_multipart_parts() {
    let (hub, mut env) = authenticated();
    env.blobs.insert(
        "blob_photo_1".into(),
        BlobInfo {
            id: "blob_photo_1".into(),
            size: 1024,
            content_type: Some("image/jpeg".into()),
            file_name: Some("camera.jpg".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );

    let props = schema::defaults("send");
    let photo = blob_media_val("media.photo", None, Some("blob_photo_1"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);

    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");

    let parts = body.as_multipart().expect("multipart body");
    let part_names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(part_names, vec!["chat_id", "photo", "disable_notification"]);

    let chat_part = parts.iter().find(|p| p.name == "chat_id").unwrap();
    assert_eq!(chat_part.data, PartData::Text("-100".into()));

    let photo_part = parts.iter().find(|p| p.name == "photo").unwrap();
    assert_eq!(photo_part.data, PartData::Blob("blob_photo_1".into()));
    assert_eq!(photo_part.file_name.as_deref(), Some("camera.jpg"));
    assert_eq!(photo_part.content_type.as_deref(), Some("image/jpeg"));

    let notif_part = parts
        .iter()
        .find(|p| p.name == "disable_notification")
        .unwrap();
    assert_eq!(notif_part.data, PartData::Text("false".into()));
}

#[test]
fn send_outgoing_media_blob_caption_and_keyboard() {
    let (hub, mut env) = authenticated();
    env.blobs.insert(
        "blob_doc_1".into(),
        BlobInfo {
            id: "blob_doc_1".into(),
            size: 2048,
            content_type: Some("application/pdf".into()),
            file_name: None,
            created: "2026-11-16T00:00:00Z".into(),
        },
    );

    let props = schema::defaults("send");
    let doc = blob_media_val("media.document", None, Some("blob_doc_1"), None);
    let inline_kb = StructValue::new("telegram.keyboard")
        .field("kind", "inline")
        .field(
            "rows",
            DataValue::List {
                element_type: DataType::Any,
                elements: vec![DataValue::List {
                    element_type: DataType::Any,
                    elements: vec![
                        StructValue::new("telegram.button")
                            .field("text", "Download")
                            .field("action", "callback")
                            .field("data", "dl_1")
                            .build(),
                    ],
                }],
            },
        )
        .build();

    let outgoing = StructValue::new("telegram.outgoing")
        .field("text", "<b>Report file</b>")
        .field("media", doc)
        .field("keyboard", inline_kb)
        .field("parse_mode", "html")
        .field("silent", true)
        .build();

    let values = inputs(&[(1, (-100i64).into()), (3, outgoing)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendDocument");

    let parts = body.as_multipart().expect("multipart body");
    let doc_part = parts.iter().find(|p| p.name == "document").unwrap();
    assert_eq!(doc_part.data, PartData::Blob("blob_doc_1".into()));
    assert_eq!(doc_part.file_name.as_deref(), Some("file.pdf"));
    assert_eq!(doc_part.content_type.as_deref(), Some("application/pdf"));

    let caption_part = parts.iter().find(|p| p.name == "caption").unwrap();
    assert_eq!(
        caption_part.data,
        PartData::Text("<b>Report file</b>".into())
    );

    let parse_mode_part = parts.iter().find(|p| p.name == "parse_mode").unwrap();
    assert_eq!(parse_mode_part.data, PartData::Text("HTML".into()));

    let silent_part = parts
        .iter()
        .find(|p| p.name == "disable_notification")
        .unwrap();
    assert_eq!(silent_part.data, PartData::Text("true".into()));

    let kb_part = parts.iter().find(|p| p.name == "reply_markup").unwrap();
    assert_eq!(
        kb_part.data,
        PartData::Text(
            json!({
                "inline_keyboard": [[{"callback_data": "dl_1", "text": "Download"}]]
            })
            .to_string()
        )
    );
}

#[test]
fn send_media_source_preference_file_id_blob_url() {
    let (hub, mut env) = authenticated();
    env.blobs.insert(
        "blob_pref".into(),
        BlobInfo {
            id: "blob_pref".into(),
            size: 512,
            content_type: Some("image/jpeg".into()),
            file_name: Some("photo.jpg".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );
    let props = schema::defaults("send");

    // 1. file_id + blob + url -> file_id wins (JSON)
    let m1 = blob_media_val(
        "media.photo",
        Some("fid_123"),
        Some("blob_pref"),
        Some("https://example.com/ignored.jpg"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, m1)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");
    assert!(body.as_multipart().is_none());
    assert_eq!(body["photo"], "fid_123");

    // 2. blob + url -> blob wins (Multipart)
    let m2 = blob_media_val(
        "media.photo",
        None,
        Some("blob_pref"),
        Some("https://example.com/ignored.jpg"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, m2)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");
    let parts = body.as_multipart().expect("multipart body");
    let photo_part = parts.iter().find(|p| p.name == "photo").unwrap();
    assert_eq!(photo_part.data, PartData::Blob("blob_pref".into()));

    // 3. url only -> url wins (JSON)
    let m3 = blob_media_val(
        "media.photo",
        None,
        None,
        Some("https://example.com/real.jpg"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, m3)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");
    assert!(body.as_multipart().is_none());
    assert_eq!(body["photo"], "https://example.com/real.jpg");
}

#[test]
fn send_blob_size_limit_errors() {
    let (hub, mut env) = authenticated();
    let props = schema::defaults("send");

    // Photo over 10 MB limit
    env.blobs.insert(
        "blob_photo_oversized".into(),
        BlobInfo {
            id: "blob_photo_oversized".into(),
            size: 10 * 1024 * 1024 + 1,
            content_type: Some("image/jpeg".into()),
            file_name: Some("big.jpg".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );
    let photo = blob_media_val("media.photo", None, Some("blob_photo_oversized"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);
    let err = body::build("send", &props, &values, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-upload-size");
    assert_eq!(err.args.first().map(|a| a.1.as_str()), Some("10"));

    // Document over 50 MB limit
    env.blobs.insert(
        "blob_doc_oversized".into(),
        BlobInfo {
            id: "blob_doc_oversized".into(),
            size: 50 * 1024 * 1024 + 1,
            content_type: Some("application/zip".into()),
            file_name: Some("big.zip".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );
    let doc = blob_media_val("media.document", None, Some("blob_doc_oversized"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, doc)]);
    let err = body::build("send", &props, &values, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-upload-size");
    assert_eq!(err.args.first().map(|a| a.1.as_str()), Some("50"));

    // Photo exactly 10 MB: ok
    env.blobs.insert(
        "blob_photo_ok".into(),
        BlobInfo {
            id: "blob_photo_ok".into(),
            size: 10 * 1024 * 1024,
            content_type: Some("image/jpeg".into()),
            file_name: Some("ok.jpg".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );
    let photo_ok = blob_media_val("media.photo", None, Some("blob_photo_ok"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo_ok)]);
    assert!(body::build("send", &props, &values, &hub, &env).is_ok());
}

#[test]
fn send_blob_evicted_error() {
    let (hub, env) = authenticated();
    let props = schema::defaults("send");

    let photo = blob_media_val("media.photo", None, Some("unknown_blob"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);
    let err = body::build("send", &props, &values, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-blob-evicted");
    assert_eq!(
        err.fallback,
        "Media blob was evicted or not found in storage"
    );
}

#[test]
fn send_multipart_429_retry_resends_multipart() {
    let (mut hub, mut env) = authenticated();
    env.requests.clear();
    env.blobs.insert(
        "blob_retry".into(),
        BlobInfo {
            id: "blob_retry".into(),
            size: 1024,
            content_type: Some("image/jpeg".into()),
            file_name: Some("pic.jpg".into()),
            created: "2026-11-16T00:00:00Z".into(),
        },
    );

    let mut node = ActionNode::new("send");
    let photo = blob_media_val("media.photo", None, Some("blob_retry"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);

    // 1. Trigger Pin starts multipart HTTP task
    let out = node.run(Trigger::Pin(PinId(0)), &values, 1000, &mut hub, &mut env);
    assert!(out.is_empty());
    assert_eq!(env.requests.len(), 1);
    let task_id = env.requests[0].0;
    let first_multipart = env.requests[0].1.multipart.clone();

    // 2. Telegram returns 429 Too Many Requests
    let out = node.run(
        Trigger::Task {
            id: task_id,
            result: response(
                429,
                json!({
                    "ok": false,
                    "error_code": 429,
                    "description": "Too Many Requests",
                    "parameters": {"retry_after": 2}
                }),
            ),
        },
        &values,
        1001,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty());
    assert_eq!(env.delays.len(), 1);
    let delay_task_id = env.delays[0].0;
    assert_eq!(env.delays[0].1, 2);

    // 3. Delay completes -> resends identical multipart request
    let out = node.run(
        Trigger::Task {
            id: delay_task_id,
            result: Ok(TaskOutput::Delay),
        },
        &values,
        1003,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty());
    assert_eq!(env.requests.len(), 2);
    let retry_req = &env.requests[1].1;
    assert_eq!(retry_req.multipart, first_multipart);

    // 4. Retry succeeds with message
    let msg_json = json!({
        "message_id": 99,
        "date": 1700000000,
        "chat": {"id": -100, "type": "supergroup", "title": "Group"},
        "photo": [{"file_id": "p1", "file_unique_id": "u1", "width": 100, "height": 100}]
    });
    let out = node.run(
        Trigger::Task {
            id: env.requests[1].0,
            result: response(200, json!({"ok": true, "result": msg_json})),
        },
        &values,
        1004,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(10))));
    assert!(out.contains(&Output::Data(PinId(14), 99i64.into())));
}
