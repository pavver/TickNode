use crate::tests::*;
use crate::{Telegram, api, schema, validation};
use serde_json::json;
use ticknode_plugin_sdk::*;

#[test]
fn editor_contracts_keep_pins_costs_purity_and_input_property_bindings_stable() {
    for (kind, pure, cost) in [
        ("message", false, 2),
        ("command", false, 2),
        ("callback", false, 2),
        ("send", false, 50),
        ("edit", false, 50),
        ("delete", false, 50),
        ("download", false, 50),
        ("button", true, 1),
        ("keyboard", true, 1),
        ("compose", true, 1),
    ] {
        let definition = schema::definition(kind);
        assert_eq!(definition.pure, pure, "{kind}");
        assert_eq!(definition.cost, cost, "{kind}");
        assert!(!definition.adaptive, "{kind}");
        let node = Telegram::create(&format!("telegram/{kind}"), NodeId::generate()).unwrap();
        assert_eq!(node.definition(), definition);
        let expected: &[(u32, &str, PinDirection)] = match kind {
            "button" => &[
                (0, "Button", PinDirection::Output),
                (1, "Text", PinDirection::Input),
            ],
            "keyboard" => &[
                (0, "Keyboard", PinDirection::Output),
                (1, "Button 1", PinDirection::Input),
            ],
            "compose" => &[
                (0, "Message", PinDirection::Output),
                (1, "Text", PinDirection::Input),
            ],
            "download" => &[
                (0, "In", PinDirection::Input),
                (1, "Media", PinDirection::Input),
            ],
            "command" => &[
                (0, "Received", PinDirection::Output),
                (1, "Arguments", PinDirection::Output),
            ],
            "message" => &[
                (0, "Received", PinDirection::Output),
                (1, "Text", PinDirection::Output),
            ],
            "callback" => &[
                (0, "Pressed", PinDirection::Output),
                (1, "Data", PinDirection::Output),
            ],
            _ => &[
                (0, "In", PinDirection::Input),
                (1, "Chat", PinDirection::Input),
            ],
        };
        for (pin, name, direction) in expected {
            assert!(
                definition
                    .pins
                    .iter()
                    .any(|p| p.id == PinId(*pin) && p.name == *name && p.direction == *direction),
                "{kind}: {pin}/{name}"
            );
        }
        if ["send", "edit", "delete"].contains(&kind) {
            assert_eq!(
                definition
                    .properties
                    .iter()
                    .find(|p| p.name == "chat")
                    .unwrap()
                    .input,
                Some(PinId(1))
            );
            assert!(
                definition
                    .pins
                    .iter()
                    .any(|p| p.id == PinId(1) && p.required)
            );
        }
    }
}

#[test]
fn edited_messages_keep_existing_buttons_by_default() {
    assert_eq!(schema::defaults("edit")["keep_buttons"], "yes");
    assert_eq!(schema::defaults("send")["disable_preview"], "no");
}

#[test]
fn api_response_requires_both_success_status_and_ok_flag() {
    for (status, ok) in [(200, false), (500, true), (500, false)] {
        assert!(api::response(response(status, json!({"ok": ok, "result": {"id": 1}}))).is_err());
    }
    assert_eq!(
        api::response(response(200, json!({"ok":true,"result":{"id":1}}))).unwrap(),
        json!({"id":1})
    );
    for result in [
        Err("https://botSECRET/failure".into()),
        Ok(TaskOutput::Delay),
    ] {
        let failure = api::response(result).unwrap_err();
        assert_eq!(failure.code, 0);
        assert!(!failure.description.contains("SECRET"));
    }
}

#[test]
fn command_names_and_argument_splitting_keep_empty_words_and_escaping() {
    assert!(validation::command(&"a".repeat(32)).is_some());
    assert!(validation::command(&"a".repeat(33)).is_none());
    assert_eq!(validation::args(""), Vec::<String>::new());
    assert_eq!(validation::args("  one  two "), ["one", "two"]);
    assert_eq!(validation::args("'' \"\" \\ "), ["", "", " "]);
    assert_eq!(
        validation::args("one\\ two three\\"),
        ["one two", "three\\"]
    );
}

#[test]
fn untyped_outgoing_values_and_default_notification_flags_are_supported() {
    use crate::outgoing::*;
    let value = DataValue::Object {
        type_name: None,
        fields: match StructValue::new("unused").field("text", "hello").build() {
            DataValue::Object { fields, .. } => fields,
            _ => unreachable!(),
        },
    };
    let outgoing = parse_outgoing(&value).unwrap();
    assert_eq!(outgoing.text.as_deref(), Some("hello"));
    assert!(!outgoing.silent);
    assert_eq!(
        parse_outgoing(&StructValue::new("telegram.outgoing").build())
            .unwrap_err()
            .key,
        "telegram-error-compose-empty"
    );
    assert_eq!(
        parse_outgoing(&StructValue::new("unknown").build())
            .unwrap_err()
            .key,
        "telegram-error-outgoing-value"
    );
}
