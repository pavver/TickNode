//! Outgoing message types, pure builder definitions, and property defaults.
use super::named;
use std::collections::BTreeMap;
use ticknode_plugin_sdk::*;

pub fn types() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition::new_enum(
            "telegram.parse_mode",
            "Parse mode",
            &[
                ("plain", "Plain text"),
                ("html", "HTML"),
                ("markdown_v2", "MarkdownV2"),
            ],
        )
        .described("Telegram text formatting mode"),
        TypeDefinition::new_enum(
            "telegram.button_action",
            "Button action",
            &[
                ("callback", "Callback"),
                ("url", "URL"),
                ("text", "Text"),
                ("contact", "Share contact"),
                ("location", "Share location"),
            ],
        )
        .described("Action performed when the button is pressed"),
        TypeDefinition::new_struct(
            "telegram.button",
            "Telegram button",
            vec![
                FieldDefinition::new("text", "Text", DataType::String),
                FieldDefinition::new("action", "Action", named("button_action")),
                FieldDefinition::new("data", "Data", DataType::String).optional(),
            ],
        )
        .described("A Telegram keyboard button"),
        TypeDefinition::new_enum(
            "telegram.keyboard_kind",
            "Keyboard kind",
            &[
                ("inline", "Inline"),
                ("reply", "Reply"),
                ("remove", "Remove"),
            ],
        )
        .described("Telegram keyboard layout kind"),
        TypeDefinition::new_struct(
            "telegram.keyboard",
            "Telegram keyboard",
            vec![
                FieldDefinition::new("kind", "Kind", named("keyboard_kind")),
                FieldDefinition::new(
                    "rows",
                    "Rows",
                    DataType::List(Box::new(DataType::List(Box::new(named("button"))))),
                ),
                FieldDefinition::new("resize", "Resize", DataType::Bool).optional(),
                FieldDefinition::new("one_time", "One-time", DataType::Bool).optional(),
                FieldDefinition::new("placeholder", "Placeholder", DataType::String).optional(),
                FieldDefinition::new("persistent", "Persistent", DataType::Bool).optional(),
            ],
        )
        .described("A Telegram custom keyboard"),
        TypeDefinition::new_struct(
            "telegram.outgoing",
            "Outgoing message",
            vec![
                FieldDefinition::new("text", "Text", DataType::String).optional(),
                FieldDefinition::new("parse_mode", "Parse mode", named("parse_mode")).optional(),
                FieldDefinition::new("media", "Media", DataType::Any).optional(),
                FieldDefinition::new("keyboard", "Keyboard", named("keyboard")).optional(),
                FieldDefinition::new("silent", "Silent", DataType::Bool).optional(),
            ],
        )
        .described("An outgoing Telegram message value"),
    ]
}

pub fn button_definition() -> NodeDefinition {
    NodeDefinition {
        type_name: "telegram/button".into(),
        title: "Button".into(),
        category: "Telegram".into(),
        description: "Build a Telegram keyboard button".into(),
        help: "Creates a button for the Keyboard node. Text is required for the button label. Select an action: Callback (for inline keyboards, sends callback data or the button text if Data is empty), URL (for inline keyboards, opens a web link from Data), Text (for reply keyboards, sends text), or Share contact / Share location (for reply keyboards). Connect Button to a Keyboard input.".into(),
        pins: vec![
            data_out(0, "Button", named("button")),
            data_in(1, "Text", DataType::String).required(),
            data_in(2, "Data", DataType::String),
        ],
        properties: vec![PropertyDefinition::select(
            "action",
            "Action",
            &[
                ("callback", "Callback"),
                ("url", "URL"),
                ("text", "Text"),
                ("contact", "Share contact"),
                ("location", "Share location"),
            ],
        )],
        adaptive: false,
        pure: true,
        cost: 1,
    }
}

pub fn keyboard_definition(props: &BTreeMap<String, String>) -> NodeDefinition {
    let count: u32 = props
        .get("count")
        .and_then(|s| s.parse().ok())
        .unwrap_or(3)
        .clamp(1, 24);
    let mut pins = vec![data_out(0, "Keyboard", named("keyboard"))];
    for i in 1..=count {
        pins.push(data_in(i, &format!("Button {i}"), DataType::Any));
    }

    let is_reply = props.get("kind").map(String::as_str) == Some("reply");
    let mut properties = vec![
        PropertyDefinition::select(
            "kind",
            "Keyboard kind",
            &[
                ("inline", "Inline"),
                ("reply", "Reply"),
                ("remove", "Remove"),
            ],
        ),
        PropertyDefinition::text("count", "Buttons").described("Number of button inputs (1–24)"),
        PropertyDefinition::text("columns", "Columns").described("Buttons per row (1–8)"),
    ];

    if is_reply {
        properties.extend([
            PropertyDefinition::select(
                "resize",
                "Resize keyboard",
                &[("yes", "Yes"), ("no", "No")],
            ),
            PropertyDefinition::select(
                "one_time",
                "One-time keyboard",
                &[("no", "No"), ("yes", "Yes")],
            ),
            PropertyDefinition::text("placeholder", "Input placeholder"),
            PropertyDefinition::select(
                "persistent",
                "Persistent keyboard",
                &[("no", "No"), ("yes", "Yes")],
            ),
        ]);
    } else {
        properties.extend([
            PropertyDefinition::hidden("resize"),
            PropertyDefinition::hidden("one_time"),
            PropertyDefinition::hidden("placeholder"),
            PropertyDefinition::hidden("persistent"),
        ]);
    }

    NodeDefinition {
        type_name: "telegram/keyboard".into(),
        title: "Keyboard".into(),
        category: "Telegram".into(),
        description: "Build a Telegram keyboard layout".into(),
        help: "Combines button values or plain text into an inline, reply or remove keyboard layout. Buttons sets the number of input pins, and Columns arranges them into rows left to right. Button inputs accept a button from the Button node or plain text (converted to a callback button for inline keyboards or a text button for reply keyboards). Empty inputs are skipped. Connect Keyboard to Compose message.".into(),
        pins,
        properties,
        adaptive: false,
        pure: true,
        cost: 1,
    }
}

pub fn compose_definition() -> NodeDefinition {
    NodeDefinition {
        type_name: "telegram/compose".into(),
        title: "Compose message".into(),
        category: "Telegram".into(),
        description: "Compose an outgoing Telegram message with text, media and keyboard".into(),
        help: "Combines text, media and a keyboard into an outgoing message value for Send message or Edit message. Provide text (up to 4096 characters without media, or a caption up to 1024 characters with media) or media (photo, audio, video or document). Formatting is selected via parse mode.".into(),
        pins: vec![
            data_out(0, "Message", named("outgoing")),
            data_in(1, "Text", DataType::String),
            data_in(2, "Media", DataType::Any),
            data_in(3, "Keyboard", named("keyboard")),
        ],
        properties: vec![
            PropertyDefinition::select(
                "parse_mode",
                "Parse mode",
                &[
                    ("plain", "Plain text"),
                    ("html", "HTML"),
                    ("markdown_v2", "MarkdownV2"),
                ],
            ),
            PropertyDefinition::select(
                "silent",
                "Silent",
                &[("no", "No"), ("yes", "Yes")],
            ),
        ],
        adaptive: false,
        pure: true,
        cost: 1,
    }
}

pub fn definition(kind: &str, props: &BTreeMap<String, String>) -> NodeDefinition {
    match kind {
        "button" => button_definition(),
        "keyboard" => keyboard_definition(props),
        "compose" => compose_definition(),
        _ => panic!("unknown compose node kind: {kind}"),
    }
}

pub fn defaults(kind: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    match kind {
        "button" => {
            map.insert("action".into(), "callback".into());
        }
        "keyboard" => {
            map.insert("kind".into(), "inline".into());
            map.insert("count".into(), "3".into());
            map.insert("columns".into(), "1".into());
            map.insert("resize".into(), "yes".into());
            map.insert("one_time".into(), "no".into());
            map.insert("placeholder".into(), String::new());
            map.insert("persistent".into(), "no".into());
        }
        "compose" => {
            map.insert("parse_mode".into(), "plain".into());
            map.insert("silent".into(), "no".into());
        }
        _ => {}
    }
    map
}
