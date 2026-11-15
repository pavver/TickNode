use super::*;
use crate::tests::*;
use serde_json::json;

fn inputs(values: &[(u32, DataValue)]) -> Inputs {
    values
        .iter()
        .map(|(id, v)| (PinId(*id), v.clone()))
        .collect()
}

pub(crate) fn media_val(type_name: &str, file_id: Option<&str>, url: Option<&str>) -> DataValue {
    let mut s = StructValue::new(type_name);
    if let Some(fid) = file_id {
        s = s.field("file_id", fid);
    }
    if let Some(u) = url {
        s = s.field("url", u);
    }
    s.build()
}

#[test]
fn send_media_methods_and_bodies_prefer_file_id_over_url() {
    let (mut hub, _) = authenticated();
    let mut original = message(-100, 50, "hello");
    original["message_thread_id"] = 7.into();
    hub.remember(&serde_json::from_value(original).unwrap());

    // 1. Photo: file_id preferred over url
    let mut node = ActionNode::new("send_media");
    node.props.insert("kind".into(), "photo".into());
    node.props.insert("parse_mode".into(), "HTML".into());
    node.props.insert("silent".into(), "yes".into());
    node.props
        .insert("buttons".into(), "Open=https://example.org".into());

    let photo = media_val(
        "media.photo",
        Some("tg_photo_123"),
        Some("https://example.com/ignored.jpg"),
    );
    let values = inputs(&[
        (1, (-100i64).into()),
        (3, photo),
        (4, "<b>Sunset</b>".into()),
        (5, 50i64.into()),
    ]);
    let (method, body) = body::build("send_media", &node.props, &values, &hub).unwrap();
    assert_eq!(method, "sendPhoto");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["photo"], "tg_photo_123");
    assert_eq!(body["caption"], "<b>Sunset</b>");
    assert_eq!(body["parse_mode"], "HTML");
    assert_eq!(body["disable_notification"], true);
    assert_eq!(body["message_thread_id"], 7);
    assert_eq!(body["reply_parameters"], json!({"message_id": 50}));
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][0]["url"],
        "https://example.org"
    );

    // 2. Audio: file_id
    let mut audio_node = ActionNode::new("send_media");
    audio_node.props.insert("kind".into(), "audio".into());
    let audio = media_val("media.audio", Some("tg_audio_456"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, audio)]);
    let (method, body) = body::build("send_media", &audio_node.props, &values, &hub).unwrap();
    assert_eq!(method, "sendAudio");
    assert_eq!(body["audio"], "tg_audio_456");

    // 3. Video: file_id
    let mut video_node = ActionNode::new("send_media");
    video_node.props.insert("kind".into(), "video".into());
    let video = media_val("media.video", Some("tg_vid_789"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, video)]);
    let (method, body) = body::build("send_media", &video_node.props, &values, &hub).unwrap();
    assert_eq!(method, "sendVideo");
    assert_eq!(body["video"], "tg_vid_789");

    // 4. Document: url when file_id absent
    let mut doc_node = ActionNode::new("send_media");
    doc_node.props.insert("kind".into(), "document".into());
    let doc = media_val(
        "media.document",
        None,
        Some("https://example.com/report.pdf"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, doc)]);
    let (method, body) = body::build("send_media", &doc_node.props, &values, &hub).unwrap();
    assert_eq!(method, "sendDocument");
    assert_eq!(body["document"], "https://example.com/report.pdf");
}

#[test]
fn send_media_caption_limit() {
    let (hub, _) = authenticated();
    let props = schema::defaults("send_media");
    let photo = media_val("media.photo", Some("tg_photo"), None);

    for (len, ok) in [(0, true), (1024, true), (1025, false)] {
        let values = inputs(&[
            (1, (-100i64).into()),
            (3, photo.clone()),
            (4, "я".repeat(len).into()),
        ]);
        let res = body::build("send_media", &props, &values, &hub);
        assert_eq!(res.is_ok(), ok, "failed for length {len}");
        if !ok {
            assert_eq!(res.unwrap_err().key, "telegram-error-caption");
        }
    }
}

#[test]
fn send_media_missing_source_error() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send_media");
    let empty_media = StructValue::new("media.photo").build();
    let values = inputs(&[(1, (-100i64).into()), (3, empty_media)]);

    let err = body::build("send_media", &node.props, &values, &hub).unwrap_err();
    assert_eq!(err.key, "telegram-error-media");

    let outputs = node.run(
        Trigger::Pin(PinId(0)),
        &values,
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    assert!(outputs.contains(&Output::Exec(PinId(11))));
    assert_eq!(node.error.unwrap().key, "telegram-error-media");
}

#[test]
fn send_media_429_rate_limit_retries_and_succeeds() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send_media");
    let photo = media_val("media.photo", Some("tg_photo_1"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);

    assert!(
        node.run(
            Trigger::Pin(PinId(0)),
            &values,
            1_700_000_000,
            &mut hub,
            &mut env
        )
        .is_empty()
    );

    let (req_id, _) = env.requests.last().unwrap().clone();
    // Telegram returns 429 retry_after: 8
    assert!(
        node.run(
            Trigger::Task {
                id: req_id,
                result: response(
                    429,
                    json!({"ok": false, "error_code": 429, "parameters": {"retry_after": 8}})
                ),
            },
            &Inputs::default(),
            1_700_000_001,
            &mut hub,
            &mut env,
        )
        .is_empty()
    );

    assert_eq!(env.delays.last().unwrap().1, 8);
    let delay_id = env.delays.last().unwrap().0;

    // Delay fires, node resubmits
    assert!(
        node.run(
            Trigger::Task {
                id: delay_id,
                result: Ok(TaskOutput::Delay),
            },
            &Inputs::default(),
            1_700_000_009,
            &mut hub,
            &mut env,
        )
        .is_empty()
    );

    let new_req_id = env.requests.last().unwrap().0;
    let success = node.run(
        Trigger::Task {
            id: new_req_id,
            result: response(
                200,
                json!({"ok": true, "result": message(-100, 99, "caption")}),
            ),
        },
        &Inputs::default(),
        1_700_000_010,
        &mut hub,
        &mut env,
    );

    assert!(success.contains(&Output::Exec(PinId(10))));
    assert!(success.contains(&Output::Data(PinId(14), 99i64.into())));
}

#[test]
fn send_media_kind_switch_keeps_pins_and_unaffected_connections() {
    let mut node = ActionNode::new("send_media");
    let def_photo = node.definition();
    assert_eq!(
        def_photo.pins[3].pin_type,
        PinType::Data(DataType::Named("media.photo".into()))
    );

    // Switch kind to "video"
    node.set_property("kind", DataValue::String("video".into()));
    let def_video = node.definition();
    assert_eq!(
        def_video.pins[3].pin_type,
        PinType::Data(DataType::Named("media.video".into()))
    );

    // All pin IDs, names and other pin types are identical
    assert_eq!(def_photo.pins.len(), def_video.pins.len());
    for i in [0, 1, 2, 4, 5, 6, 7, 8, 9, 10] {
        assert_eq!(def_photo.pins[i].id, def_video.pins[i].id);
        assert_eq!(def_photo.pins[i].name, def_video.pins[i].name);
        assert_eq!(def_photo.pins[i].pin_type, def_video.pins[i].pin_type);
        assert_eq!(def_photo.pins[i].direction, def_video.pins[i].direction);
    }
}

#[test]
fn incoming_message_extracts_photo_audio_video_document_and_caption() {
    let raw = json!({
        "message_id": 101,
        "chat": {"id": -100, "type": "supergroup", "title": "Media Chat"},
        "date": 1_700_000_000,
        "caption": "Check this out!",
        "photo": [
            {"file_id": "thumb_1", "width": 100, "height": 100, "file_size": 1000},
            {"file_id": "large_1", "width": 1920, "height": 1080, "file_size": 250000}
        ],
        "audio": {
            "file_id": "aud_1",
            "duration": 180,
            "file_name": "song.mp3",
            "mime_type": "audio/mpeg",
            "file_size": 5000000
        },
        "video": {
            "file_id": "vid_1",
            "width": 1280,
            "height": 720,
            "duration": 60,
            "file_name": "clip.mp4",
            "mime_type": "video/mp4",
            "file_size": 15000000
        },
        "document": {
            "file_id": "doc_1",
            "file_name": "paper.pdf",
            "mime_type": "application/pdf",
            "file_size": 800000
        }
    });

    let msg: Message = serde_json::from_value(raw).unwrap();
    let val = msg.value();

    // Caption used as text
    assert_eq!(
        field(&val, "text"),
        Some(&DataValue::String("Check this out!".into()))
    );

    // Photo selects largest
    let photo = field(&val, "photo").expect("photo present");
    assert_eq!(field_string(photo, "file_id"), Some("large_1"));
    assert_eq!(field_number(photo, "width"), Some(1920.0));
    assert_eq!(field_number(photo, "height"), Some(1080.0));
    assert_eq!(field_number(photo, "size"), Some(250000.0));

    // Audio
    let audio = field(&val, "audio").expect("audio present");
    assert_eq!(field_string(audio, "file_id"), Some("aud_1"));
    assert_eq!(field_string(audio, "file_name"), Some("song.mp3"));

    // Video
    let video = field(&val, "video").expect("video present");
    assert_eq!(field_string(video, "file_id"), Some("vid_1"));
    assert_eq!(field_number(video, "width"), Some(1280.0));

    // Document
    let doc = field(&val, "document").expect("doc present");
    assert_eq!(field_string(doc, "file_id"), Some("doc_1"));
    assert_eq!(field_string(doc, "file_name"), Some("paper.pdf"));
}
