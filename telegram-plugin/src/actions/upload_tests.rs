use super::{tests::*, *};
use crate::tests::*;
use serde_json::{Value, json};

fn blob(env: &mut Fake, kind: &str, size: u64) -> DataValue {
    env.blobs.insert(
        "upload".into(),
        BlobInfo {
            id: "upload".into(),
            size,
            content_type: Some("application/octet-stream".into()),
            file_name: Some("stored.bin".into()),
            created: String::new(),
        },
    );
    StructValue::new(&format!("media.{kind}"))
        .field("blob", "upload")
        .build()
}

#[test]
fn multipart_and_json_send_have_identical_nonmedia_parameters() {
    let (hub, mut env) = authenticated();
    let media = blob(&mut env, "photo", 100);
    let message = |media| {
        StructValue::new("telegram.outgoing")
            .field("media", media)
            .field("text", "Caption")
            .field("parse_mode", "markdown_v2")
            .field("silent", true)
            .field(
                "keyboard",
                StructValue::new("telegram.keyboard")
                    .field("kind", "remove")
                    .build(),
            )
            .build()
    };
    let build = |media| {
        body::build(
            "send",
            &schema::defaults("send"),
            &inputs(&[
                (1, (-100i64).into()),
                (2, 9i64.into()),
                (3, message(media)),
                (4, 50i64.into()),
            ]),
            &hub,
            &env,
        )
        .unwrap()
        .1
    };
    let json_body = build(
        StructValue::new("media.photo")
            .field("file_id", "telegram-file")
            .build(),
    );
    let multipart_body = build(media);
    let mut normalized = serde_json::Map::new();
    for part in multipart_body.as_multipart().unwrap() {
        if part.name == "photo" {
            continue;
        }
        let PartData::Text(text) = &part.data else {
            panic!("Expected text parameter");
        };
        normalized.insert(
            part.name.clone(),
            match part.name.as_str() {
                "caption" | "parse_mode" => Value::String(text.clone()),
                _ => serde_json::from_str(text).unwrap(),
            },
        );
    }
    assert_eq!(normalized["reply_parameters"], json!({"message_id": 50}));
    assert_eq!(normalized["message_thread_id"], 9);
    let api::RequestBody::Json(Value::Object(mut original)) = json_body else {
        panic!("Expected JSON");
    };
    original.remove("photo");
    assert_eq!(normalized, original);
}

#[test]
fn upload_metadata_prefers_media_then_blob_then_kind_and_mime_defaults() {
    let (hub, mut env) = authenticated();
    let media = blob(&mut env, "document", 100);
    let overridden = StructValue::new("media.document")
        .field("blob", "upload")
        .field("file_name", "explicit.pdf")
        .field("mime_type", "application/pdf")
        .build();
    for (value, name, mime) in [
        (overridden, "explicit.pdf", "application/pdf"),
        (media, "stored.bin", "application/octet-stream"),
    ] {
        let (_, body) = body::build(
            "send",
            &schema::defaults("send"),
            &inputs(&[(1, (-100i64).into()), (3, value)]),
            &hub,
            &env,
        )
        .unwrap();
        let part = body
            .as_multipart()
            .unwrap()
            .iter()
            .find(|p| p.name == "document")
            .unwrap();
        assert_eq!(part.file_name.as_deref(), Some(name));
        assert_eq!(part.content_type.as_deref(), Some(mime));
    }
    env.blobs.get_mut("upload").unwrap().file_name = None;
    env.blobs.get_mut("upload").unwrap().content_type = None;
    for (kind, name) in [
        ("photo", "photo.jpg"),
        ("audio", "audio.mp3"),
        ("video", "video.mp4"),
        ("document", "file.bin"),
    ] {
        let (_, body) = body::build(
            "send",
            &schema::defaults("send"),
            &inputs(&[
                (1, (-100i64).into()),
                (
                    3,
                    StructValue::new(&format!("media.{kind}"))
                        .field("blob", "upload")
                        .build(),
                ),
            ]),
            &hub,
            &env,
        )
        .unwrap();
        let part = body
            .as_multipart()
            .unwrap()
            .iter()
            .find(|p| matches!(p.data, PartData::Blob(_)))
            .unwrap();
        assert_eq!(part.name, kind);
        assert_eq!(part.file_name.as_deref(), Some(name));
        assert_eq!(part.content_type, None);
    }
}

#[test]
fn upload_limits_and_eviction_fail_node_without_any_http_request() {
    for (kind, limit) in [
        ("photo", 10),
        ("audio", 50),
        ("video", 50),
        ("document", 50),
    ] {
        let (mut hub, mut env) = authenticated();
        env.requests.clear();
        let media = blob(&mut env, kind, limit * 1024 * 1024 + 1);
        let values = inputs(&[(1, (-100i64).into()), (3, media)]);
        let mut node = ActionNode::new("send");
        let out = node.run(Trigger::Pin(PinId(0)), &values, 0, &mut hub, &mut env);
        assert!(out.contains(&Output::Exec(PinId(11))));
        assert!(out.iter().any(|o| matches!(o, Output::Data(PinId(12), _))));
        assert_eq!(
            node.error.as_ref().unwrap().key,
            "telegram-error-upload-size"
        );
        assert!(env.requests.is_empty());
        env.blobs.get_mut("upload").unwrap().size -= 1;
        assert!(body::build("send", &schema::defaults("send"), &values, &hub, &env).is_ok());
        env.blobs.clear();
        let out = node.run(Trigger::Pin(PinId(0)), &values, 0, &mut hub, &mut env);
        assert!(out.contains(&Output::Exec(PinId(11))));
        assert_eq!(
            node.error.as_ref().unwrap().key,
            "telegram-error-blob-evicted"
        );
        assert!(env.requests.is_empty());
    }
}
