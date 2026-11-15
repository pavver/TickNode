use super::tests::*;
use super::*;
use crate::tests::*;
use serde_json::json;

fn make_outgoing(
    text: Option<&str>,
    media: Option<DataValue>,
    keyboard: Option<DataValue>,
    parse_mode: Option<&str>,
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
                            .field("text", "Click")
                            .field("action", "callback")
                            .field("data", "click_1")
                            .build(),
                    ],
                }],
            },
        )
        .build()
}

#[test]
fn edit_plain_text_and_defaults() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 42i64.into()),
        (3, "Edited content".into()),
    ]);

    let (method, body) = body::build("edit", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "editMessageText");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["message_id"], 42);
    assert_eq!(body["text"], "Edited content");
    assert!(body.get("parse_mode").is_none());
    // keep_buttons defaults to "yes": no reply_markup sent when no keyboard
    assert!(body.get("reply_markup").is_none());
}

#[test]
fn edit_with_outgoing_text_and_parse_mode() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");
    let outgoing = make_outgoing(Some("*Bold text*"), None, None, Some("markdown_v2"));
    let values = inputs(&[(1, (-100i64).into()), (2, 42i64.into()), (3, outgoing)]);

    let (method, body) = body::build("edit", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "editMessageText");
    assert_eq!(body["text"], "*Bold text*");
    assert_eq!(body["parse_mode"], "MarkdownV2");
}

#[test]
fn edit_with_inline_keyboard() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");
    let outgoing = make_outgoing(
        Some("Updated text"),
        None,
        Some(inline_keyboard_val()),
        None,
    );
    let values = inputs(&[(1, (-100i64).into()), (2, 42i64.into()), (3, outgoing)]);

    let (method, body) = body::build("edit", &props, &values, &hub, &env).unwrap();
    assert_eq!(method, "editMessageText");
    assert_eq!(
        body["reply_markup"]["inline_keyboard"][0][0]["callback_data"],
        "click_1"
    );
}

#[test]
fn edit_keep_buttons_yes_and_no() {
    let (hub, env) = authenticated();
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 42i64.into()),
        (3, "New text".into()),
    ]);

    // keep_buttons = yes (default) -> no reply_markup
    let mut props_yes = schema::defaults("edit");
    props_yes.insert("keep_buttons".into(), "yes".into());
    let (_, body_yes) = body::build("edit", &props_yes, &values, &hub, &env).unwrap();
    assert!(body_yes.get("reply_markup").is_none());

    // keep_buttons = no -> empty inline_keyboard to remove buttons
    let mut props_no = schema::defaults("edit");
    props_no.insert("keep_buttons".into(), "no".into());
    let (_, body_no) = body::build("edit", &props_no, &values, &hub, &env).unwrap();
    assert_eq!(body_no["reply_markup"], json!({"inline_keyboard": []}));
}

#[test]
fn edit_with_media_returns_translated_error() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");

    // Bare media
    let photo = media_val("media.photo", Some("file_123"), None);
    let values_bare = inputs(&[(1, (-100i64).into()), (2, 42i64.into()), (3, photo.clone())]);
    let err = body::build("edit", &props, &values_bare, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-edit-media");
    assert_eq!(err.fallback, "Editing message media is not supported");

    // Outgoing with media
    let outgoing = make_outgoing(Some("caption"), Some(photo), None, None);
    let values_outgoing = inputs(&[(1, (-100i64).into()), (2, 42i64.into()), (3, outgoing)]);
    let err = body::build("edit", &props, &values_outgoing, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-edit-media");
}

#[test]
fn edit_with_reply_or_remove_keyboard_returns_translated_error() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");

    // Reply keyboard
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
                            .field("text", "Text")
                            .field("action", "text")
                            .build(),
                    ],
                }],
            },
        )
        .build();
    let outgoing_reply = make_outgoing(Some("text"), None, Some(reply_kb), None);
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 42i64.into()),
        (3, outgoing_reply),
    ]);
    let err = body::build("edit", &props, &values, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-edit-keyboard");
    assert_eq!(
        err.fallback,
        "Edited messages support only inline keyboards"
    );

    // Remove keyboard
    let remove_kb = StructValue::new("telegram.keyboard")
        .field("kind", "remove")
        .build();
    let outgoing_remove = make_outgoing(Some("text"), None, Some(remove_kb), None);
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 42i64.into()),
        (3, outgoing_remove),
    ]);
    let err = body::build("edit", &props, &values, &hub, &env).unwrap_err();
    assert_eq!(err.key, "telegram-error-edit-keyboard");
}

#[test]
fn edit_validation_errors() {
    let (hub, env) = authenticated();
    let props = schema::defaults("edit");

    // Missing message id
    let values = inputs(&[(1, (-100i64).into()), (3, "text".into())]);
    assert_eq!(
        body::build("edit", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-message"
    );

    // Empty text
    let values = inputs(&[(1, (-100i64).into()), (2, 42i64.into()), (3, "".into())]);
    assert_eq!(
        body::build("edit", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-text"
    );

    // Oversized text
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 42i64.into()),
        (3, "x".repeat(4097).into()),
    ]);
    assert_eq!(
        body::build("edit", &props, &values, &hub, &env)
            .unwrap_err()
            .key,
        "telegram-error-length"
    );
}
