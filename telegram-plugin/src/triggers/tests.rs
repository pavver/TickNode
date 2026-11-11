use super::*;
use crate::tests::*;
use serde_json::json;
fn filter(kind: &str, values: &[(&str, &str)]) -> Filter {
    let mut props = schema::defaults(kind);
    for (k, v) in values {
        props.insert((*k).into(), (*v).into());
    }
    Filter::new(kind, props).unwrap()
}
fn parsed(value: serde_json::Value) -> crate::model::Update {
    serde_json::from_value(value).unwrap()
}
#[test]
fn message_filters_chat_topic_user_and_text_modes() {
    let mut raw = update(1, "hello world");
    raw["message"]["message_thread_id"] = 7.into();
    let update = parsed(raw);
    for (mode, pattern) in [
        ("any", ""),
        ("equals", "hello world"),
        ("contains", "world"),
        ("starts_with", "hello"),
        ("regex", r"^hello\s+world$"),
    ] {
        assert!(
            filter(
                "message",
                &[
                    ("chat", "-100"),
                    ("topic", "7"),
                    ("user", "17"),
                    ("text_mode", mode),
                    ("pattern", pattern)
                ]
            )
            .outputs(&update, "Bot")
            .is_some()
        );
    }
    for (key, value) in [
        ("chat", "42"),
        ("topic", "8"),
        ("user", "42"),
        ("pattern", "absent"),
    ] {
        assert!(
            filter("message", &[("text_mode", "contains"), (key, value)])
                .outputs(&update, "Bot")
                .is_none()
        );
    }
}
#[test]
fn edited_messages_are_opt_in_and_never_commands() {
    let raw = json!({"update_id":1,"edited_message":message(-100,50,"/status")});
    let update = parsed(raw);
    assert!(filter("message", &[]).outputs(&update, "Bot").is_none());
    assert!(
        filter("message", &[("include_edited", "yes")])
            .outputs(&update, "Bot")
            .is_some()
    );
    assert!(
        filter("command", &[("command", "status")])
            .outputs(&update, "Bot")
            .is_none()
    );
}
#[test]
fn commands_accept_slash_bot_mention_quotes_and_reject_other_bots() {
    let update = parsed(update(
        1,
        "/status@TickBot   \"living room\" 'two words' one\\ two \"\"",
    ));
    let outputs = filter("command", &[("command", "/status")])
        .outputs(&update, "tickbot")
        .unwrap();
    let args = outputs
        .iter()
        .find_map(|o| match o {
            Output::Data(PinId(2), DataValue::List { elements, .. }) => Some(elements),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        args,
        &vec![
            "living room".into(),
            "two words".into(),
            "one two".into(),
            "".into()
        ]
    );
    assert!(
        filter("command", &[("command", "status")])
            .outputs(&update, "OtherBot")
            .is_none()
    );
    for name in [
        "",
        "Status",
        "a-b",
        "a b",
        "/",
        "abcdefghijklmnopqrstuvwxyz1234567",
    ] {
        assert!(validation::command(name).is_none());
    }
}
#[test]
fn callback_projects_the_clicker_and_clears_absent_inline_message() {
    let update = parsed(
        json!({"update_id":1,"callback_query":{"id":"q","data":"status:1","from":{"id":23,"first_name":"Clicker"}}}),
    );
    assert!(
        filter(
            "callback",
            &[("pattern", "status"), ("data_mode", "equals")]
        )
        .outputs(&update, "Bot")
        .is_none()
    );
    let outputs = filter(
        "callback",
        &[("pattern", "status:"), ("data_mode", "prefix")],
    )
    .outputs(&update, "Bot")
    .unwrap();
    assert!(outputs.contains(&Output::Clear(PinId(3))));
    assert!(outputs.contains(&Output::Clear(PinId(4))));
    assert!(
        outputs
            .iter()
            .any(|o| matches!(o,Output::Data(PinId(6),v) if field(v,"id")==Some(&23i64.into())))
    );
}
#[test]
fn missing_optional_fields_clear_outputs_between_messages() {
    let update = parsed(update(1, "text"));
    let outputs = filter("message", &[]).outputs(&update, "Bot").unwrap();
    assert!(outputs.contains(&Output::Clear(PinId(5))));
    let mut raw = crate::tests::update(2, "");
    raw["message"].as_object_mut().unwrap().remove("text");
    assert!(
        filter("message", &[])
            .outputs(&parsed(raw), "Bot")
            .unwrap()
            .contains(&Output::Clear(PinId(1)))
    );
}
#[test]
fn invalid_restored_regex_or_command_is_rejected() {
    let mut props = schema::defaults("message");
    props.insert("text_mode".into(), "regex".into());
    props.insert("pattern".into(), "[".into());
    assert!(Filter::new("message", props).is_err());
    assert!(Filter::new("command", schema::defaults("command")).is_err());
}
