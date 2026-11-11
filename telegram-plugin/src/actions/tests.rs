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
fn send_body_has_parse_mode_silent_previews_buttons_reply_and_inferred_topic() {
    let (mut hub, _) = authenticated();
    let mut original = message(-100, 50, "/status");
    original["message_thread_id"] = 7.into();
    hub.remember(&serde_json::from_value(original).unwrap());
    let mut node = ActionNode::new("send");
    for (key, value) in [
        ("parse_mode", "HTML"),
        ("silent", "yes"),
        ("disable_preview", "yes"),
        (
            "buttons",
            "Refresh=status | Open=https://example.org\nOther=other",
        ),
    ] {
        node.props.insert(key.into(), value.into());
    }
    let values = inputs(&[
        (1, (-100i64).into()),
        (3, "<b>Status</b>".into()),
        (4, 50i64.into()),
    ]);
    let (method, body) = body::build("send", &node.props, &values, &hub).unwrap();
    assert_eq!(method, "sendMessage");
    assert_eq!(body["chat_id"], -100);
    assert_eq!(body["message_thread_id"], 7);
    assert_eq!(body["reply_parameters"], json!({"message_id":50}));
    assert_eq!(body["parse_mode"], "HTML");
    assert_eq!(body["disable_notification"], true);
    assert_eq!(body["link_preview_options"]["is_disabled"], true);
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
fn explicit_topic_overrides_reply_and_chat_property_replaces_input() {
    let (hub, _) = authenticated();
    let mut props = schema::defaults("send");
    props.insert("chat".into(), "42".into());
    let (_, body) = body::build(
        "send",
        &props,
        &inputs(&[
            (1, (-100i64).into()),
            (2, 9i64.into()),
            (3, "hi".into()),
            (4, 50i64.into()),
        ]),
        &hub,
    )
    .unwrap();
    assert_eq!(body["chat_id"], 42);
    assert_eq!(body["message_thread_id"], 9);
}
#[test]
fn edit_and_delete_bodies_use_chat_message_id_and_optional_parse_mode() {
    let (hub, _) = authenticated();
    let mut props = schema::defaults("edit");
    props.insert("parse_mode".into(), "MarkdownV2".into());
    let values = inputs(&[
        (1, (-100i64).into()),
        (2, 51i64.into()),
        (3, "Updated".into()),
    ]);
    assert_eq!(
        body::build("edit", &props, &values, &hub).unwrap(),
        (
            "editMessageText",
            json!({"chat_id":-100,"message_id":51,"text":"Updated","parse_mode":"MarkdownV2"})
        )
    );
    assert_eq!(
        body::build("delete", &schema::defaults("delete"), &values, &hub).unwrap(),
        ("deleteMessage", json!({"chat_id":-100,"message_id":51}))
    );
}
#[test]
fn text_limit_counts_unicode_characters_without_truncation() {
    let (hub, _) = authenticated();
    let props = schema::defaults("send");
    for (length, ok) in [(0, false), (4096, true), (4097, false)] {
        let values = inputs(&[(1, (-100i64).into()), (3, "я".repeat(length).into())]);
        assert_eq!(body::build("send", &props, &values, &hub).is_ok(), ok);
    }
}
#[test]
fn buttons_reject_empty_labels_invalid_links_and_oversized_callback_bytes() {
    for text in [
        "Missing equal",
        "=data",
        "Label=",
        "Label=http://example.org",
        "Label=https://",
        "Label=https://bad url",
    ] {
        assert!(validation::buttons(text).is_err(), "{text}");
    }
    assert!(validation::buttons(&format!("Label={}", "я".repeat(33))).is_err());
    assert!(validation::buttons(&format!("Label={}", "я".repeat(32))).is_ok());
}
#[test]
fn allowlist_refuses_send_edit_delete_with_node_error_and_no_http() {
    let (mut hub, mut env) = authenticated();
    hub.allowed.insert(42);
    for kind in ["send", "edit", "delete"] {
        let mut node = ActionNode::new(kind);
        let before = env.requests.len();
        let output = node.run(
            Trigger::Pin(PinId(0)),
            &sending(),
            1_700_000_000,
            &mut hub,
            &mut env,
        );
        assert!(output.contains(&Output::Exec(PinId(11))));
        assert_eq!(node.error.unwrap().key, "telegram-error-forbidden");
        assert_eq!(env.requests.len(), before);
    }
}
#[test]
fn send_success_returns_typed_message_id_and_done() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send");
    assert!(
        node.run(
            Trigger::Pin(PinId(0)),
            &sending(),
            1_700_000_000,
            &mut hub,
            &mut env
        )
        .is_empty()
    );
    let id = env.requests.last().unwrap().0;
    let output = node.run(
        Trigger::Task {
            id,
            result: response(200, json!({"ok":true,"result":message(-100,51,"Hello")})),
        },
        &Inputs::default(),
        1_700_000_001,
        &mut hub,
        &mut env,
    );
    assert!(output.contains(&Output::Data(PinId(14), 51i64.into())));
    assert!(output.contains(&Output::Exec(PinId(10))));
    assert!(output.iter().any(
        |o| matches!(o,Output::Data(PinId(13),v) if field(v,"chat")==Some(&(-100i64).into()))
    ));
}
#[test]
fn rate_limit_waits_retry_after_then_resubmits_the_original_body_as_node_task() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send");
    node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    let (id, original) = env.requests.last().unwrap().clone();
    assert!(
        node.run(
            Trigger::Task {
                id,
                result: response(
                    429,
                    json!({"ok":false,"error_code":429,"parameters":{"retry_after":12}})
                )
            },
            &Inputs::default(),
            1_700_000_001,
            &mut hub,
            &mut env
        )
        .is_empty()
    );
    assert_eq!(env.delays.last().unwrap().1, 12);
    assert_eq!(env.requests.last().unwrap().0, id);
    let delay = env.delays.last().unwrap().0;
    node.run(
        Trigger::Task {
            id: delay,
            result: Ok(TaskOutput::Delay),
        },
        &Inputs::default(),
        1_700_000_013,
        &mut hub,
        &mut env,
    );
    assert_eq!(env.requests.last().unwrap().1, original);
}
#[test]
fn unauthorized_action_reports_failed_and_settings_error_on_plugin_tick() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send");
    node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    let id = env.requests.last().unwrap().0;
    let out = node.run(
        Trigger::Task {
            id,
            result: response(401, json!({"ok":false,"error_code":401})),
        },
        &Inputs::default(),
        1_700_000_001,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(
        node.error.as_ref().unwrap().key,
        "telegram-error-unauthorized"
    );
    hub.tick(1_700_000_002, vec![], &mut env);
    assert_eq!(env.errors[0].key, "telegram-error-unauthorized");
}
#[test]
fn settings_change_during_retry_never_sends_with_new_credentials() {
    let (mut hub, mut env) = authenticated();
    let mut node = ActionNode::new("send");
    node.run(
        Trigger::Pin(PinId(0)),
        &sending(),
        1_700_000_000,
        &mut hub,
        &mut env,
    );
    let id = env.requests.last().unwrap().0;
    hub.generation += 1;
    let out = node.run(
        Trigger::Task {
            id,
            result: response(200, json!({"ok":true,"result":message(-100,51,"hi")})),
        },
        &Inputs::default(),
        1_700_000_001,
        &mut hub,
        &mut env,
    );
    assert!(out.contains(&Output::Exec(PinId(11))));
    assert_eq!(node.error.unwrap().key, "telegram-error-changed");
}

#[test]
fn reply_to_a_known_topic_root_uses_the_persisted_directory_without_a_message_cache() {
    let (mut hub, _) = authenticated();
    let mut raw = message(-100, 7, "");
    raw["forum_topic_created"] = json!({"name":"Kitchen"});
    hub.directory
        .message(&serde_json::from_value(raw).unwrap(), 1_700_000_000);
    let (_, body) = body::build(
        "send",
        &schema::defaults("send"),
        &inputs(&[(1, (-100i64).into()), (3, "Reply".into()), (4, 7i64.into())]),
        &hub,
    )
    .unwrap();
    assert_eq!(body["message_thread_id"], 7);
}
