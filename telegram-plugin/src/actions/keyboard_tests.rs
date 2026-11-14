//! Tests for Telegram chat reply keyboards, options, and edit buttons.
use super::*;
use crate::tests::*;
use serde_json::json;

fn inputs(values: &[(u32, DataValue)]) -> Inputs {
    values
        .iter()
        .map(|(id, v)| (PinId(*id), v.clone()))
        .collect()
}

fn sending() -> Inputs {
    inputs(&[(1, (-100i64).into()), (3, "Hello".into())])
}

#[test]
fn send_body_supports_all_keyboard_modes() {
    let (hub, _) = authenticated();
    let values = inputs(&[(1, (-100i64).into()), (3, "Hello".into())]);

    // 1. Inline keyboard (default) with buttons
    let mut inline_props = schema::defaults("send");
    inline_props.insert("keyboard".into(), "inline".into());
    inline_props.insert("buttons".into(), "Btn=data".into());
    let (_, body) = body::build("send", &inline_props, &values, &hub).unwrap();
    assert_eq!(
        body["reply_markup"]["inline_keyboard"],
        json!([[{"text": "Btn", "callback_data": "data"}]])
    );

    // 2. Inline keyboard without buttons: no reply_markup
    let mut empty_inline = schema::defaults("send");
    empty_inline.insert("keyboard".into(), "inline".into());
    let (_, body) = body::build("send", &empty_inline, &values, &hub).unwrap();
    assert!(body.get("reply_markup").is_none());

    // 3. Reply keyboard with default options (resize=yes, one_time=no, persistent=no)
    let mut reply_props = schema::defaults("send");
    reply_props.insert("keyboard".into(), "reply".into());
    reply_props.insert(
        "buttons".into(),
        "Status | Contact=contact\nLocation=location".into(),
    );
    let (_, body) = body::build("send", &reply_props, &values, &hub).unwrap();
    assert_eq!(
        body["reply_markup"]["keyboard"],
        json!([
            [{"text": "Status"}, {"text": "Contact", "request_contact": true}],
            [{"text": "Location", "request_location": true}]
        ])
    );
    assert_eq!(body["reply_markup"]["resize_keyboard"], true);
    assert_eq!(body["reply_markup"]["one_time_keyboard"], false);
    assert_eq!(body["reply_markup"]["is_persistent"], false);
    assert!(
        body["reply_markup"]
            .get("input_field_placeholder")
            .is_none()
    );

    // 4. Reply keyboard with custom options
    let mut custom_reply = schema::defaults("send");
    custom_reply.insert("keyboard".into(), "reply".into());
    custom_reply.insert("buttons".into(), "Menu".into());
    custom_reply.insert("resize".into(), "no".into());
    custom_reply.insert("one_time".into(), "yes".into());
    custom_reply.insert("persistent".into(), "yes".into());
    custom_reply.insert("placeholder".into(), "Choose an option".into());
    let (_, body) = body::build("send", &custom_reply, &values, &hub).unwrap();
    assert_eq!(body["reply_markup"]["resize_keyboard"], false);
    assert_eq!(body["reply_markup"]["one_time_keyboard"], true);
    assert_eq!(body["reply_markup"]["is_persistent"], true);
    assert_eq!(
        body["reply_markup"]["input_field_placeholder"],
        "Choose an option"
    );

    // 5. Remove keyboard mode
    let mut remove_props = schema::defaults("send");
    remove_props.insert("keyboard".into(), "remove".into());
    let (_, body) = body::build("send", &remove_props, &values, &hub).unwrap();
    assert_eq!(body["reply_markup"], json!({"remove_keyboard": true}));

    // 6. None keyboard mode
    let mut none_props = schema::defaults("send");
    none_props.insert("keyboard".into(), "none".into());
    none_props.insert("buttons".into(), "Ignored=data".into());
    let (_, body) = body::build("send", &none_props, &values, &hub).unwrap();
    assert!(body.get("reply_markup").is_none());
}

#[test]
fn send_media_supports_keyboard_modes() {
    let (hub, _) = authenticated();
    let media = StructValue::new("media.photo")
        .field("file_id", "photo123".to_string())
        .build();
    let values = inputs(&[(1, (-100i64).into()), (3, media), (4, "Caption".into())]);

    let mut reply_props = schema::defaults("send_media");
    reply_props.insert("keyboard".into(), "reply".into());
    reply_props.insert("buttons".into(), "Share=contact".into());
    let (_, body) = body::build("send_media", &reply_props, &values, &hub).unwrap();
    assert_eq!(
        body["reply_markup"]["keyboard"],
        json!([[{"text": "Share", "request_contact": true}]])
    );

    let mut remove_props = schema::defaults("send_media");
    remove_props.insert("keyboard".into(), "remove".into());
    let (_, body) = body::build("send_media", &remove_props, &values, &hub).unwrap();
    assert_eq!(body["reply_markup"], json!({"remove_keyboard": true}));
}

#[test]
fn edit_message_with_and_without_buttons_and_keep_buttons_choice() {
    let (hub, _) = authenticated();
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 51i64.into()),
        (3, "Updated text".into()),
    ]);

    // 1. Default edit: keep_buttons=yes and no buttons -> no reply_markup in body (existing behaviour)
    let default_props = schema::defaults("edit");
    let (_, body) = body::build("edit", &default_props, &values, &hub).unwrap();
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["message_id"], 51);
    assert_eq!(body["text"], "Updated text");
    assert!(body.get("reply_markup").is_none());

    // 2. Edit with new inline buttons replaces keyboard
    let mut with_buttons = schema::defaults("edit");
    with_buttons.insert("buttons".into(), "Click=test".into());
    let (_, body) = body::build("edit", &with_buttons, &values, &hub).unwrap();
    assert_eq!(
        body["reply_markup"]["inline_keyboard"],
        json!([[{"text": "Click", "callback_data": "test"}]])
    );

    // 3. Edit with keep_buttons=no and empty buttons explicitly removes keyboard
    let mut remove_buttons = schema::defaults("edit");
    remove_buttons.insert("keep_buttons".into(), "no".into());
    let (_, body) = body::build("edit", &remove_buttons, &values, &hub).unwrap();
    assert_eq!(body["reply_markup"], json!({"inline_keyboard": []}));
}

#[test]
fn reply_keyboard_validation_checks_syntax_labels_and_limits() {
    let mut reply_props = Properties::new();
    reply_props.insert("keyboard".into(), "reply".into());

    // Valid buttons
    assert!(validation::property("send", "buttons", "Plain | Test", &reply_props).is_ok());
    assert!(validation::property("send", "buttons", "Contact=contact", &reply_props).is_ok());
    assert!(validation::property("send", "buttons", "Where=location", &reply_props).is_ok());

    // Invalid buttons: empty, invalid action, empty label
    assert_eq!(
        validation::property("send", "buttons", "", &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-reply-buttons"
    );
    assert_eq!(
        validation::property("send", "buttons", "Label=callback_data", &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-reply-buttons"
    );
    assert_eq!(
        validation::property("send", "buttons", "=contact", &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-reply-buttons"
    );
    assert_eq!(
        validation::property("send", "buttons", "A | | B", &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-reply-buttons"
    );

    // Placeholder limit 64 characters
    assert!(validation::property("send", "placeholder", &"a".repeat(64), &reply_props).is_ok());
    assert_eq!(
        validation::property("send", "placeholder", &"a".repeat(65), &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-placeholder"
    );

    // Keyboard choice validation
    assert!(validation::property("send", "keyboard", "reply", &reply_props).is_ok());
    assert!(validation::property("send", "keyboard", "remove", &reply_props).is_ok());
    assert!(validation::property("send", "keyboard", "none", &reply_props).is_ok());
    assert_eq!(
        validation::property("send", "keyboard", "unknown", &reply_props)
            .unwrap_err()
            .key,
        "telegram-error-choice"
    );

    // Options choice validation
    for opt in ["resize", "one_time", "persistent"] {
        assert!(validation::property("send", opt, "yes", &reply_props).is_ok());
        assert!(validation::property("send", opt, "no", &reply_props).is_ok());
        assert_eq!(
            validation::property("send", opt, "maybe", &reply_props)
                .unwrap_err()
                .key,
            "telegram-error-choice"
        );
    }

    // Edit keep_buttons validation
    let edit_props = Properties::new();
    assert!(validation::property("edit", "keep_buttons", "yes", &edit_props).is_ok());
    assert!(validation::property("edit", "keep_buttons", "no", &edit_props).is_ok());
    assert_eq!(
        validation::property("edit", "keep_buttons", "invalid", &edit_props)
            .unwrap_err()
            .key,
        "telegram-error-choice"
    );
}

#[test]
fn restored_invalid_values_raise_translated_runtime_errors() {
    let (mut hub, mut env) = authenticated();

    // 1. Restored Send node with keyboard = reply but empty buttons
    let mut node = ActionNode::new("send");
    node.props.insert("keyboard".into(), "reply".into());
    node.props.insert("buttons".into(), "".into());
    let out = node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(
        node.error.as_ref().unwrap().key,
        "telegram-error-reply-buttons"
    );

    // 2. Restored Send node with invalid keyboard choice
    let mut node = ActionNode::new("send");
    node.props.insert("keyboard".into(), "bogus".into());
    let out = node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(node.error.as_ref().unwrap().key, "telegram-error-choice");

    // 3. Restored Send node with oversized placeholder
    let mut node = ActionNode::new("send");
    node.props.insert("keyboard".into(), "reply".into());
    node.props.insert("buttons".into(), "Hello".into());
    node.props.insert("placeholder".into(), "x".repeat(65));
    let out = node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(
        node.error.as_ref().unwrap().key,
        "telegram-error-placeholder"
    );

    // 4. Restored Edit node with invalid keep_buttons
    let mut node = ActionNode::new("edit");
    node.props.insert("keep_buttons".into(), "bad".into());
    let values = inputs(&[(1, (-100i64).into()), (2, 51i64.into()), (3, "text".into())]);
    let out = node.run(
        Trigger::Pin(PinId(0)),
        &values,
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(node.error.as_ref().unwrap().key, "telegram-error-choice");
}
