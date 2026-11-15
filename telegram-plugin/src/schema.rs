//! Stable positional contracts and English definition fallbacks.
use ticknode_plugin_sdk::*;

pub fn named(name: &str) -> DataType {
    DataType::Named(format!("telegram.{name}"))
}
pub fn types() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition::identifier("telegram.chat_id", "Telegram chat ID", "A Telegram chat")
            .labels("chats"),
        TypeDefinition::identifier("telegram.user_id", "Telegram user ID", "A Telegram user")
            .labels("users"),
        TypeDefinition::identifier(
            "telegram.message_id",
            "Telegram message ID",
            "A message ID within a chat",
        ),
        TypeDefinition::identifier(
            "telegram.topic_id",
            "Telegram topic ID",
            "A topic ID within a chat",
        )
        .labels("topics"),
        TypeDefinition::new_struct(
            "telegram.user",
            "Telegram user",
            vec![
                FieldDefinition::new("id", "ID", named("user_id")),
                FieldDefinition::new("first_name", "First name", DataType::String),
                FieldDefinition::new("last_name", "Last name", DataType::String).optional(),
                FieldDefinition::new("username", "Username", DataType::String).optional(),
                FieldDefinition::new("language", "Language", DataType::String).optional(),
                FieldDefinition::new("is_bot", "Is bot", DataType::Bool),
            ],
        )
        .described("The sender of a Telegram message or callback"),
        TypeDefinition::new_struct(
            "telegram.message",
            "Telegram message",
            vec![
                FieldDefinition::new("chat", "Chat", named("chat_id")),
                FieldDefinition::new("topic", "Topic", named("topic_id")).optional(),
                FieldDefinition::new("id", "ID", named("message_id")),
                FieldDefinition::new("from", "From", named("user")).optional(),
                FieldDefinition::new("text", "Text", DataType::String).optional(),
                FieldDefinition::new("date", "Date", DataType::DateTime),
                FieldDefinition::new("reply_to", "Reply to", named("message_id")).optional(),
                FieldDefinition::new("chat_title", "Chat title", DataType::String).optional(),
                FieldDefinition::new("chat_type", "Chat type", DataType::String),
                FieldDefinition::new("photo", "Photo", DataType::Named("media.photo".into()))
                    .optional(),
                FieldDefinition::new("audio", "Audio", DataType::Named("media.audio".into()))
                    .optional(),
                FieldDefinition::new("video", "Video", DataType::Named("media.video".into()))
                    .optional(),
                FieldDefinition::new("document", "File", DataType::Named("media.document".into()))
                    .optional(),
            ],
        )
        .described("A Telegram message with chat, topic, sender, text and media"),
    ]
}

fn media_type_for_kind(kind: Option<&str>) -> &'static str {
    match kind {
        Some("audio") => "media.audio",
        Some("video") => "media.video",
        Some("document") => "media.document",
        _ => "media.photo",
    }
}

pub fn definition(kind: &str) -> NodeDefinition {
    action_definition(kind, &std::collections::BTreeMap::new())
}

pub fn action_definition(
    kind: &str,
    props: &std::collections::BTreeMap<String, String>,
) -> NodeDefinition {
    let trigger = matches!(kind, "message" | "command" | "callback");
    let (title, description, help) = match kind {
        "message" => (
            "New Message",
            "Receive Telegram messages",
            "Emits one message per tick. Optional chat, topic and user filters combine. Choose a text mode and pattern (pressing a reply-keyboard button sends an ordinary message matching Equals mode); enable edited messages to receive edits. Connect Chat and Text to Send message to echo a message.",
        ),
        "command" => (
            "Command",
            "Receive a Telegram bot command",
            "Enter status or /status. Accepts /status@YourBot with the verified bot username. Arguments is the remaining text; Args splits whitespace and respects quotes. To reply to /status, connect Received to Send message, Chat to Chat and a Text from template output to Text; connect Message through Break Telegram message to Reply to.",
        ),
        "callback" => (
            "Button Pressed",
            "Receive inline keyboard callbacks",
            "Filter callback data by equality or prefix. Every callback is answered automatically, even without a matching node. Toast text is optional. For a button Refresh=status, filter status and connect Pressed to your action. Message can be absent for inline callbacks.",
        ),
        "send" => (
            "Send Message",
            "Send text and inline buttons to Telegram",
            "Connect In, Chat and Text. Leave the Chat property empty to use its input. For /status replies, wire Command Chat to Chat and Text from template to Text. Reply to keeps the original topic when known; Topic overrides it. Choose keyboard mode: inline, reply, remove or none. Inline buttons: Refresh=status | Open=https://example.org. Reply buttons: plain text, Text=contact or Text=location. Text is limited to 4096 characters. Done and Message are emitted on success; Failed and Error on failure.",
        ),
        "send_media" => (
            "Send Media",
            "Send photo, audio, video or document to Telegram",
            "Connect In, Chat and Media. Select kind (photo, audio, video or document): the Media pin type follows the kind. Optional Caption allows up to 1024 characters. Send file_id when present, else url. Reply to keeps the original topic when known; Topic overrides it. Choose keyboard mode: inline, reply, remove or none. Done and Message are emitted on success; Failed and Error on failure.",
        ),
        "edit" => (
            "Edit Message",
            "Replace a Telegram message's text",
            "Connect In, Chat, Message id and Text. Use Message id from Send message or Break Telegram message. Select none, HTML or MarkdownV2. Optional inline buttons replace the keyboard; Keep buttons controls whether editing text keeps existing buttons when none are specified. Done means the edit succeeded; Failed and Error describe a failure. Text is limited to 4096 characters.",
        ),
        _ => (
            "Delete Message",
            "Delete a Telegram message",
            "Connect In, Chat and Message id, for example from Send message. Telegram permissions and deletion time limits apply. Done means deletion succeeded; Failed and Error describe a failure.",
        ),
    };
    let mut properties = Vec::new();
    let pins = if trigger {
        properties.extend([
            PropertyDefinition::dynamic("chat", "Chat", "chats"),
            PropertyDefinition::dynamic("topic", "Topic", "topics"),
            PropertyDefinition::dynamic("user", "User", "users"),
        ]);
        if kind == "message" {
            properties.extend([
                PropertyDefinition::select(
                    "text_mode",
                    "Text mode",
                    &[
                        ("any", "Any"),
                        ("equals", "Equals"),
                        ("contains", "Contains"),
                        ("starts_with", "Starts with"),
                        ("regex", "Regular expression"),
                    ],
                ),
                PropertyDefinition::text("pattern", "Pattern"),
                yes_no("include_edited", "Include edited messages"),
            ]);
        } else if kind == "command" {
            properties.push(PropertyDefinition::text("command", "Command"));
        } else {
            properties.extend([
                PropertyDefinition::select(
                    "data_mode",
                    "Data mode",
                    &[("equals", "Equals"), ("prefix", "Prefix")],
                ),
                PropertyDefinition::text("pattern", "Data filter"),
                PropertyDefinition::text("toast", "Toast text"),
            ]);
        }
        let mut pins = vec![exec_out(
            0,
            if kind == "callback" {
                "Pressed"
            } else {
                "Received"
            },
        )];
        if kind == "command" {
            pins.extend([
                data_out(1, "Arguments", DataType::String),
                data_out(2, "Args", DataType::List(Box::new(DataType::String))),
            ]);
        } else {
            pins.push(data_out(
                1,
                if kind == "callback" { "Data" } else { "Text" },
                DataType::String,
            ));
        }
        pins.extend([
            data_out(3, "Message", named("message")),
            data_out(4, "Chat", named("chat_id")),
            data_out(5, "Topic", named("topic_id")),
            data_out(6, "From", named("user")),
        ]);
        pins
    } else {
        properties.push(PropertyDefinition::dynamic("chat", "Chat", "chats").or_input(1));
        let mut pins = vec![
            exec_in(0, "In"),
            data_in(1, "Chat", named("chat_id")).required(),
        ];
        if kind == "send" {
            pins.extend([
                data_in(2, "Topic", named("topic_id")),
                data_in(3, "Text", DataType::String).required(),
                data_in(4, "Reply to", named("message_id")),
            ]);
            properties.extend([
                yes_no("silent", "Silent"),
                yes_no("disable_preview", "Disable link preview"),
            ]);
            keyboard_properties(&mut properties, props);
        } else if kind == "send_media" {
            let media_type = media_type_for_kind(props.get("kind").map(String::as_str));
            pins.extend([
                data_in(2, "Topic", named("topic_id")),
                data_in(3, "Media", DataType::Named(media_type.into())).required(),
                data_in(4, "Caption", DataType::String),
                data_in(5, "Reply to", named("message_id")),
            ]);
            properties.extend([
                PropertyDefinition::select(
                    "kind",
                    "Kind",
                    &[
                        ("photo", "Photo"),
                        ("audio", "Audio"),
                        ("video", "Video"),
                        ("document", "File"),
                    ],
                ),
                yes_no("silent", "Silent"),
            ]);
            keyboard_properties(&mut properties, props);
        } else {
            pins.push(data_in(2, "Message id", named("message_id")).required());
            if kind == "edit" {
                pins.push(data_in(3, "Text", DataType::String).required());
                properties.extend([
                    PropertyDefinition::multiline("buttons", "Buttons"),
                    PropertyDefinition::select(
                        "keep_buttons",
                        "Keep buttons",
                        &[("yes", "Yes"), ("no", "No")],
                    ),
                ]);
            }
        }
        if kind != "delete" {
            properties.push(PropertyDefinition::select(
                "parse_mode",
                "Parse mode",
                &[
                    ("none", "None"),
                    ("HTML", "HTML"),
                    ("MarkdownV2", "MarkdownV2"),
                ],
            ));
        }
        pins.extend([
            exec_out(10, "Done"),
            exec_out(11, "Failed"),
            data_out(12, "Error", DataType::String),
        ]);
        if kind == "send" || kind == "send_media" {
            pins.extend([
                data_out(13, "Message", named("message")),
                data_out(14, "Message id", named("message_id")),
            ]);
        }
        pins
    };
    NodeDefinition {
        type_name: format!("telegram/{kind}"),
        title: title.into(),
        category: "Telegram".into(),
        description: description.into(),
        help: help.into(),
        pins,
        properties,
        adaptive: false,
        cost: if trigger { 2 } else { 50 },
    }
}
fn keyboard_properties(
    properties: &mut Vec<PropertyDefinition>,
    props: &std::collections::BTreeMap<String, String>,
) {
    properties.extend([
        PropertyDefinition::select(
            "keyboard",
            "Keyboard",
            &[
                ("inline", "Inline"),
                ("reply", "Reply"),
                ("remove", "Remove"),
                ("none", "None"),
            ],
        ),
        PropertyDefinition::multiline("buttons", "Buttons"),
    ]);
    if props.get("keyboard").map(String::as_str) == Some("reply") {
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
    }
}
fn yes_no(name: &str, title: &str) -> PropertyDefinition {
    PropertyDefinition::select(name, title, &[("no", "No"), ("yes", "Yes")])
}
pub fn defaults(kind: &str) -> std::collections::BTreeMap<String, String> {
    let mut map = definition(kind)
        .properties
        .into_iter()
        .map(|p| {
            let value = match &p.kind {
                PropertyKind::Select(options) => options[0].0.clone(),
                _ => String::new(),
            };
            (p.name, value)
        })
        .collect::<std::collections::BTreeMap<String, String>>();
    if kind == "send" || kind == "send_media" {
        map.entry("keyboard".into())
            .or_insert_with(|| "inline".into());
        map.entry("resize".into()).or_insert_with(|| "yes".into());
        map.entry("one_time".into()).or_insert_with(|| "no".into());
        map.entry("placeholder".into()).or_default();
        map.entry("persistent".into())
            .or_insert_with(|| "no".into());
    } else if kind == "edit" {
        map.entry("buttons".into()).or_default();
        map.entry("keep_buttons".into())
            .or_insert_with(|| "yes".into());
    }
    map
}
