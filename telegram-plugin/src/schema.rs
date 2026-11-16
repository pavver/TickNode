//! Stable positional contracts and English definition fallbacks.
pub mod outgoing;

use ticknode_plugin_sdk::*;

pub fn named(name: &str) -> DataType {
    DataType::Named(format!("telegram.{name}"))
}
pub fn types() -> Vec<TypeDefinition> {
    let mut list = vec![
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
    ];
    list.extend(outgoing::types());
    list
}

pub fn definition(kind: &str) -> NodeDefinition {
    action_definition(kind, &std::collections::BTreeMap::new())
}

pub fn action_definition(
    kind: &str,
    props: &std::collections::BTreeMap<String, String>,
) -> NodeDefinition {
    if matches!(kind, "button" | "keyboard" | "compose") {
        return outgoing::definition(kind, props);
    }
    if kind == "download" {
        return download_definition(props);
    }
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
            "Send text, media or composed message to Telegram",
            "Connect In and Chat. Message takes plain text (type it right on the node or connect Text from template), a message from Compose message (text + media + keyboard), or media directly. Leave the Chat property empty to use its input. Reply to keeps the original topic when known; Topic overrides it. Done and Message are emitted on success; Failed and Error on failure. Media sources prefer file_id, then a local blob, then URL. Connect Download media (media/fetch) to upload a camera photo from a URL. Blob uploads are limited to 10 MB for photos and 50 MB for audio, video and files; missing or evicted blobs fail before sending.",
        ),
        "edit" => (
            "Edit Message",
            "Replace a Telegram message's text or keyboard",
            "Connect In, Chat, Message id and Message. Message takes plain text (type it right on the node or connect Text from template) or a message from Compose message with text and an optional inline keyboard. Editing media or using reply keyboards is not supported. Keep buttons controls whether existing buttons remain when the message has no keyboard. Done means the edit succeeded; Failed and Error describe a failure.",
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
                data_in(3, "Message", DataType::Any).required(),
                data_in(4, "Reply to", named("message_id")),
            ]);
            properties.push(yes_no("disable_preview", "Disable link preview"));
        } else {
            pins.push(data_in(2, "Message id", named("message_id")).required());
            if kind == "edit" {
                pins.push(data_in(3, "Message", DataType::Any).required());
                properties.push(PropertyDefinition::select(
                    "keep_buttons",
                    "Keep buttons",
                    &[("yes", "Yes"), ("no", "No")],
                ));
            }
        }
        pins.extend([
            exec_out(10, "Done"),
            exec_out(11, "Failed"),
            data_out(12, "Error", DataType::String),
        ]);
        if kind == "send" {
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
        ..Default::default()
    }
}
fn download_definition(props: &std::collections::BTreeMap<String, String>) -> NodeDefinition {
    let kind = props.get("kind").map(String::as_str).unwrap_or("auto");
    let media_pin_type = if kind == "auto" {
        DataType::Any
    } else {
        DataType::Named(format!("media.{kind}"))
    };
    NodeDefinition {
        type_name: "telegram/download".into(),
        title: "Download media".into(),
        category: "Telegram".into(),
        description: "Download Telegram media into blob storage".into(),
        help: "Downloads Telegram files into local blob storage (up to 20 MB). Connect New message Message to Break object (telegram.message), then its Photo, Audio, Video or File field to Media; connect New message Received to In. Connect Done and Media to the next action to send the downloaded file elsewhere. All input fields and the media type are kept; blob is added, and missing size and MIME type come from the response. A blob without file_id passes through unchanged. URL-only media requires the Network Download media node (media/fetch). Kind sets the expected output type; Auto leaves Any and a mismatched explicit kind fails. Done indicates success; Failed and Error describe a failure.".into(),
        pins: vec![
            exec_in(0, "In"),
            data_in(1, "Media", DataType::Any).required(),
            exec_out(10, "Done"),
            exec_out(11, "Failed"),
            data_out(12, "Error", DataType::String),
            data_out(13, "Media", media_pin_type),
        ],
        properties: vec![PropertyDefinition::select(
            "kind",
            "Kind",
            &[
                ("auto", "Auto"),
                ("photo", "Photo"),
                ("audio", "Audio"),
                ("video", "Video"),
                ("document", "File"),
            ],
        ).described("Expected media type for the output pin")],
        cost: 50,
        ..Default::default()
    }
}

fn yes_no(name: &str, title: &str) -> PropertyDefinition {
    PropertyDefinition::select(name, title, &[("no", "No"), ("yes", "Yes")])
}
pub fn defaults(kind: &str) -> std::collections::BTreeMap<String, String> {
    if matches!(kind, "button" | "keyboard" | "compose") {
        return outgoing::defaults(kind);
    }
    definition(kind)
        .properties
        .into_iter()
        .map(|p| {
            let value = match &p.kind {
                PropertyKind::Select(options) => options[0].0.clone(),
                _ => String::new(),
            };
            (p.name, value)
        })
        .collect::<std::collections::BTreeMap<String, String>>()
}
