use super::tests::*;
use super::*;
use crate::tests::*;
use serde_json::json;

fn make_outgoing(
    text: Option<&str>,
    media: Option<DataValue>,
    keyboard: Option<DataValue>,
    parse_mode: Option<&str>,
    silent: bool,
) -> DataValue {
    let mut s = StructValue::new("telegram.outgoing");
    if let Some(t) = text {
        s = s.field("text", t);
    }
    if let Some(m) = media {
        s = s.field("media", m);
    }
    if let Some(k) = keyboard {
        s = s.field("keyboard", k);
    }
    if let Some(pm) = parse_mode {
        s = s.field("parse_mode", pm);
    }
    s = s.field("silent", silent);
    s.build()
}

fn inline_keyboard_val() -> DataValue {
    StructValue::new("telegram.keyboard")
        .field("kind", "inline")
        .field(
            "rows",
            DataValue::List {
                element_type: DataType::Any,
                elements: vec![DataValue::List {
                    element_type: DataType::Any,
                    elements: vec![
                        StructValue::new("telegram.button")
                            .field("text", "Refresh")
                            .field("action", "callback")
                            .field("data", "status")
                            .build(),
                        StructValue::new("telegram.button")
                            .field("text", "Open")
                            .field("action", "url")
                            .field("data", "https://example.org")
                            .build(),
                    ],
                }],
            },
        )
        .build()
}

#[test]
fn send_with_string_and_disable_preview_option() {
    let (hub, env) = authenticated();
    let mut props = schema::defaults("send");
    let values = inputs(&[(1, (-100i64).into()), (3, "Hello plain text".into())]);

    // Default preview enabled
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendMessage");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["text"], "Hello plain text");
    assert_eq!(body["disable_notification"], false);
    assert_eq!(body["link_preview_options"]["is_disabled"], false);
    assert!(body.get("parse_mode").is_none());
    assert!(body.get("reply_markup").is_none());

    // Disable preview property
    props.insert("disable_preview".into(), "yes".into());
    let (_, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(body["link_preview_options"]["is_disabled"], true);
}

#[test]
fn send_with_outgoing_parse_mode_silent_and_inferred_topic() {
    let (mut hub, env) = authenticated();
    let mut original = message(-100, 50, "/status");
    original["message_thread_id"] = 7.into();
    hub.remember(&serde_json::from_value(original).unwrap());

    let props = schema::defaults("send");
    let outgoing = make_outgoing(
        Some("<b>Status</b>"),
        None,
        Some(inline_keyboard_val()),
        Some("html"),
        true,
    );

    let values = inputs(&[(1, (-100i64).into()), (3, outgoing), (4, 50i64.into())]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendMessage");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["message_thread_id"], 7);
    assert_eq!(body["reply_parameters"], json!({"message_id": 50}));
    assert_eq!(body["parse_mode"], "HTML");
    assert_eq!(body["disable_notification"], true);
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][0]["callback_data"],
        "status"
    );
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][1]["url"],
        "https://example.org"
    );
}

#[test]
fn send_with_outgoing_reply_keyboard_and_remove_keyboard() {
    let (hub, env) = authenticated();
    let props = schema::defaults("send");

    // Reply keyboard with options
    let reply_kb = StructValue::new("telegram.keyboard")
        .field("kind", "reply")
        .field(
            "rows",
            DataValue::List {
                element_type: DataType::Any,
                elements: vec![DataValue::List {
                    element_type: DataType::Any,
                    elements: vec![
                        StructValue::new("telegram.button")
                            .field("text", "Contact")
                            .field("action", "contact")
                            .build(),
                        StructValue::new("telegram.button")
                            .field("text", "Location")
                            .field("action", "location")
                            .build(),
                    ],
                }],
            },
        )
        .field("resize", true)
        .field("one_time", true)
        .field("placeholder", "Choose...")
        .field("persistent", true)
        .build();

    let outgoing = make_outgoing(Some("Options:"), None, Some(reply_kb), None, false);
    let values = inputs(&[(1, (-100i64).into()), (3, outgoing)]);
    let (_, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(
        body["reply_markup"],
        json!({
            "keyboard": [[
                {"text": "Contact", "request_contact": true},
                {"text": "Location", "request_location": true}
            ]],
            "resize_keyboard": true,
            "one_time_keyboard": true,
            "input_field_placeholder": "Choose...",
            "is_persistent": true
        })
    );

    // Remove keyboard
    let remove_kb = StructValue::new("telegram.keyboard")
        .field("kind", "remove")
        .build();
    let outgoing_remove = make_outgoing(Some("Done"), None, Some(remove_kb), None, false);
    let values = inputs(&[(1, (-100i64).into()), (3, outgoing_remove)]);
    let (_, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(body["reply_markup"], json!({"remove_keyboard": true}));
}

#[test]
fn send_with_bare_media_each_kind_and_file_id_vs_url() {
    let (mut hub, env) = authenticated();
    let mut original = message(-100, 50, "hello");
    original["message_thread_id"] = 7.into();
    hub.remember(&serde_json::from_value(original).unwrap());
    let props = schema::defaults("send");

    // 1. Photo: file_id preferred over url
    let photo = media_val(
        "media.photo",
        Some("tg_photo_123"),
        Some("https://example.com/ignored.jpg"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, photo), (4, 50i64.into())]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["photo"], "tg_photo_123");
    assert_eq!(body["message_thread_id"], 7);
    assert_eq!(body["reply_parameters"], json!({"message_id": 50}));

    // 2. Audio: file_id
    let audio = media_val("media.audio", Some("tg_audio_456"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, audio)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendAudio");
    assert_eq!(body["audio"], "tg_audio_456");

    // 3. Video: file_id
    let video = media_val("media.video", Some("tg_vid_789"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, video)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendVideo");
    assert_eq!(body["video"], "tg_vid_789");

    // 4. Document: url when file_id absent
    let doc = media_val(
        "media.document",
        None,
        Some("https://example.com/report.pdf"),
    );
    let values = inputs(&[(1, (-100i64).into()), (3, doc)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendDocument");
    assert_eq!(body["document"], "https://example.com/report.pdf");
}

#[test]
fn send_with_outgoing_media_caption_and_keyboard() {
    let (hub, env) = authenticated();
    let props = schema::defaults("send");

    let photo = media_val("media.photo", Some("tg_photo_123"), None);
    let outgoing = make_outgoing(
        Some("<b>Sunset</b>"),
        Some(photo),
        Some(inline_keyboard_val()),
        Some("html"),
        true,
    );

    let values = inputs(&[(1, (-100i64).into()), (3, outgoing)]);
    let (method, body) = body::build("send", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "sendPhoto");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["photo"], "tg_photo_123");
    assert_eq!(body["caption"], "<b>Sunset</b>");
    assert_eq!(body["parse_mode"], "HTML");
    assert_eq!(body["disable_notification"], true);
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][0]["callback_data"],
        "status"
    );
}

#[test]
fn send_text_and_caption_length_validation() {
    let (hub, env) = authenticated();
    let props = schema::defaults("send");

    // Plain text limits
    for (length, ok) in [(0, false), (4096, true), (4097, false)] {
        let values = inputs(&[(1, (-100i64).into()), (3, "a".repeat(length).into())]);
        assert_eq!(
            body::build("send", &props, &values, &hub, &env).is_ok(),
            ok,
            "plain len={length}"
        );
    }

    // Outgoing text without media limits (1..=4096)
    let outgoing_oversized = make_outgoing(Some(&"a".repeat(4097)), None, None, None, false);
    let values = inputs(&[(1, (-100i64).into()), (3, outgoing_oversized)]);
    assert_eq!(
        body::build("send", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-length"
    );

    // Outgoing caption with media limits (0..=1024)
    let photo = media_val("media.photo", Some("tg_photo"), None);
    let outgoing_caption_ok = make_outgoing(
        Some(&"c".repeat(1024)),
        Some(photo.clone()),
        None,
        None,
        false,
    );
    let values = inputs(&[(1, (-100i64).into()), (3, outgoing_caption_ok)]);
    assert!(body::build("send", &props, &values, &hub, &env).is_ok());

    let outgoing_caption_over =
        make_outgoing(Some(&"c".repeat(1025)), Some(photo), None, None, false);
    let values = inputs(&[(1, (-100i64).into()), (3, outgoing_caption_over)]);
    assert_eq!(
        body::build("send", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-caption"
    );
}

#[test]
fn send_invalid_message_type_errors() {
    let (hub, env) = authenticated();
    let props = schema::defaults("send");

    // Number value
    let values = inputs(&[(1, (-100i64).into()), (3, 12345i64.into())]);
    assert_eq!(
        body::build("send", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-outgoing-value"
    );

    // Bare media without url or file_id
    let invalid_media = media_val("media.photo", None, None);
    let values = inputs(&[(1, (-100i64).into()), (3, invalid_media)]);
    assert_eq!(
        body::build("send", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-media"
    );

    // Empty compose (neither text nor media)
    let empty_outgoing = make_outgoing(None, None, None, None, false);
    let values = inputs(&[(1, (-100i64).into()), (3, empty_outgoing)]);
    assert_eq!(
        body::build("send", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-compose-empty"
    );
}

#[test]
fn send_media_429_rate_limit_retries_and_succeeds() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send");
    let photo = media_val("media.photo", Some("file_999"), None);
    let values = inputs(&[(1, (-100i64).into()), (3, photo)]);

    let requests_before = env.requests.len();
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
    let id = env.requests.last().unwrap().0;

    let out = node.run(
        Trigger::Task {
            id,
            result: response(
                429,
                json!({"ok": false, "error_code": 429, "parameters": {"retry_after": 2}}),
            ),
        },
        &Inputs::default(),
        1_700_000_001,
        &mut hub,
        &mut env,
    );
    assert!(out.is_empty(), "429 must schedule delay, no output");
    assert_eq!(env.delays.last().unwrap().1, 2);

    let delay_id = env.delays.last().unwrap().0;
    assert!(
        node.run(
            Trigger::Task {
                id: delay_id,
                result: Ok(TaskOutput::Delay),
            },
            &Inputs::default(),
            1_700_000_003,
            &mut hub,
            &mut env,
        )
        .is_empty()
    );
    assert_eq!(
        env.requests.len(),
        requests_before + 2,
        "must resubmit request after delay"
    );

    let req2_id = env.requests.last().unwrap().0;
    let success_msg = message(-100, 77, "photo_uploaded");
    let out = node.run(
        Trigger::Task {
            id: req2_id,
            result: response(200, json!({"ok": true, "result": success_msg})),
        },
        &Inputs::default(),
        1_700_000_004,
        &mut hub,
        &mut env,
    );
    assert!(
        out.contains(&Output::Exec(PinId(10))),
        "must emit Done on success"
    );
    assert!(out.contains(&Output::Data(PinId(14), 77i64.into())));
}
