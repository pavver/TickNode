use super::*;
use serde_json::json;

fn sample_media(kind: &str, file_id: Option<&str>, url: Option<&str>) -> DataValue {
    let mut builder = StructValue::new(&format!("media.{kind}"));
    if let Some(id) = file_id {
        builder = builder.field("file_id", id);
    }
    if let Some(u) = url {
        builder = builder.field("url", u);
    }
    builder.build()
}

#[test]
fn parse_outgoing_string() {
    let val = DataValue::String("Hello, world!".into());
    let out = parse_outgoing(&val).expect("valid outgoing");
    assert_eq!(out.text.as_deref(), Some("Hello, world!"));
    assert_eq!(out.parse_mode, ParseMode::Plain);
    assert!(out.media.is_none());
    assert!(out.keyboard.is_none());
    assert!(!out.silent);
}

#[test]
fn parse_outgoing_bare_media() {
    let photo = sample_media("photo", Some("file_123"), None);
    let out = parse_outgoing(&photo).expect("valid media outgoing");
    assert!(out.text.is_none());
    assert_eq!(out.parse_mode, ParseMode::Plain);
    let media = out.media.expect("media present");
    assert_eq!(media.kind, "photo");
    assert_eq!(media.reference, "file_123");
    assert!(out.keyboard.is_none());
    assert!(!out.silent);

    let doc = sample_media("document", None, Some("https://example.com/file.pdf"));
    let out_doc = parse_outgoing(&doc).expect("valid media outgoing");
    let media_doc = out_doc.media.expect("media present");
    assert_eq!(media_doc.kind, "document");
    assert_eq!(media_doc.reference, "https://example.com/file.pdf");
}

#[test]
fn parse_outgoing_object_complete() {
    let kb = Keyboard {
        kind: KeyboardKind::Inline,
        rows: vec![vec![Button {
            text: "Click".into(),
            action: ButtonAction::Callback,
            data: Some("btn_click".into()),
        }]],
        resize: false,
        one_time: false,
        placeholder: None,
        persistent: false,
    };
    let kb_val = StructValue::new("telegram.keyboard")
        .field("kind", "inline")
        .field(
            "rows",
            DataValue::List {
                element_type: DataType::List(Box::new(DataType::Named("telegram.button".into()))),
                elements: vec![DataValue::List {
                    element_type: DataType::Named("telegram.button".into()),
                    elements: vec![
                        StructValue::new("telegram.button")
                            .field("text", "Click")
                            .field("action", "callback")
                            .field("data", "btn_click")
                            .build(),
                    ],
                }],
            },
        )
        .build();

    let photo = sample_media("photo", Some("photo_456"), None);
    let outgoing_val = StructValue::new("telegram.outgoing")
        .field("text", "Photo caption")
        .field("parse_mode", "html")
        .field("media", photo)
        .field("keyboard", kb_val)
        .field("silent", true)
        .build();

    let out = parse_outgoing(&outgoing_val).expect("valid outgoing object");
    assert_eq!(out.text.as_deref(), Some("Photo caption"));
    assert_eq!(out.parse_mode, ParseMode::Html);
    assert_eq!(out.parse_mode.api_str(), Some("HTML"));
    let media = out.media.expect("media present");
    assert_eq!(media.kind, "photo");
    assert_eq!(media.reference, "photo_456");
    assert_eq!(out.keyboard, Some(kb));
    assert!(out.silent);
}

#[test]
fn parse_outgoing_validation_errors() {
    // Empty string
    let empty_str = DataValue::String(String::new());
    assert_eq!(
        parse_outgoing(&empty_str).unwrap_err().key,
        "telegram-error-text"
    );

    // Text > 4096
    let long_str = DataValue::String("a".repeat(4097));
    assert_eq!(
        parse_outgoing(&long_str).unwrap_err().key,
        "telegram-error-length"
    );

    // Caption > 1024
    let photo = sample_media("photo", Some("id"), None);
    let long_caption = StructValue::new("telegram.outgoing")
        .field("text", "a".repeat(1025))
        .field("media", photo)
        .build();
    assert_eq!(
        parse_outgoing(&long_caption).unwrap_err().key,
        "telegram-error-caption"
    );

    // Bare media missing file_id and url
    let bad_photo = StructValue::new("media.photo").build();
    assert_eq!(
        parse_outgoing(&bad_photo).unwrap_err().key,
        "telegram-error-media"
    );

    // Outgoing without text and media
    let empty_msg = StructValue::new("telegram.outgoing")
        .field("silent", true)
        .build();
    assert_eq!(
        parse_outgoing(&empty_msg).unwrap_err().key,
        "telegram-error-compose-empty"
    );

    // Invalid type (number, bool, etc.)
    assert_eq!(
        parse_outgoing(&DataValue::from(42)).unwrap_err().key,
        "telegram-error-outgoing-value"
    );
}

#[test]
fn keyboard_markup_inline() {
    let kb = Keyboard {
        kind: KeyboardKind::Inline,
        rows: vec![
            vec![
                Button {
                    text: "Callback Btn".into(),
                    action: ButtonAction::Callback,
                    data: Some("cb_data".into()),
                },
                Button {
                    text: "URL Btn".into(),
                    action: ButtonAction::Url,
                    data: Some("https://example.org".into()),
                },
            ],
            vec![Button {
                text: "Default Data".into(),
                action: ButtonAction::Callback,
                data: None,
            }],
        ],
        resize: false,
        one_time: false,
        placeholder: None,
        persistent: false,
    };

    let markup = keyboard_markup(&kb);
    let expected = json!({
        "inline_keyboard": [
            [
                { "text": "Callback Btn", "callback_data": "cb_data" },
                { "text": "URL Btn", "url": "https://example.org" }
            ],
            [
                { "text": "Default Data", "callback_data": "Default Data" }
            ]
        ]
    });
    assert_eq!(markup, expected);
}

#[test]
fn keyboard_markup_reply_with_options() {
    let kb = Keyboard {
        kind: KeyboardKind::Reply,
        rows: vec![
            vec![
                Button {
                    text: "Plain Text".into(),
                    action: ButtonAction::Text,
                    data: None,
                },
                Button {
                    text: "Send Contact".into(),
                    action: ButtonAction::Contact,
                    data: None,
                },
            ],
            vec![Button {
                text: "Send Location".into(),
                action: ButtonAction::Location,
                data: None,
            }],
        ],
        resize: true,
        one_time: true,
        placeholder: Some("Choose an option...".into()),
        persistent: true,
    };

    let markup = keyboard_markup(&kb);
    let expected = json!({
        "keyboard": [
            [
                { "text": "Plain Text" },
                { "text": "Send Contact", "request_contact": true }
            ],
            [
                { "text": "Send Location", "request_location": true }
            ]
        ],
        "resize_keyboard": true,
        "one_time_keyboard": true,
        "is_persistent": true,
        "input_field_placeholder": "Choose an option..."
    });
    assert_eq!(markup, expected);
}

#[test]
fn keyboard_markup_remove() {
    let kb = Keyboard {
        kind: KeyboardKind::Remove,
        rows: Vec::new(),
        resize: false,
        one_time: false,
        placeholder: None,
        persistent: false,
    };

    let markup = keyboard_markup(&kb);
    assert_eq!(markup, json!({ "remove_keyboard": true }));
}
