use crate::*;
use ticknode_shared::*;

mod execution;
mod lifecycle;
mod pins;

pub(crate) fn named(name: &str) -> DataType {
    DataType::Named(name.to_string())
}

pub(crate) fn telegram_types() -> Vec<TypeDefinition> {
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
                FieldDefinition::new("id", "ID", named("telegram.user_id")),
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
                FieldDefinition::new("chat", "Chat", named("telegram.chat_id")),
                FieldDefinition::new("topic", "Topic", named("telegram.topic_id")).optional(),
                FieldDefinition::new("id", "ID", named("telegram.message_id")),
                FieldDefinition::new("from", "From", named("telegram.user")).optional(),
                FieldDefinition::new("text", "Text", DataType::String).optional(),
                FieldDefinition::new("date", "Date", DataType::DateTime),
                FieldDefinition::new("reply_to", "Reply to", named("telegram.message_id"))
                    .optional(),
                FieldDefinition::new("chat_title", "Chat title", DataType::String).optional(),
                FieldDefinition::new("chat_type", "Chat type", DataType::String),
            ],
        )
        .described("A Telegram message with chat, topic, sender and text"),
    ]
}

pub(crate) fn ha_types() -> Vec<TypeDefinition> {
    vec![
        TypeDefinition::new_struct(
            "ha.state",
            "HA State",
            vec![
                FieldDefinition::new("entity_id", "Entity ID", DataType::String),
                FieldDefinition::new("state", "State", DataType::String)
                    .described("The state text reported by Home Assistant"),
                FieldDefinition::new("attributes", "Attributes", DataType::Object),
                FieldDefinition::new("last_changed", "Last changed", DataType::DateTime),
                FieldDefinition::new("last_updated", "Last updated", DataType::DateTime),
            ],
        )
        .described("The state of a Home Assistant entity"),
    ]
}

pub(crate) fn setup_engine() -> TickEngine {
    let mut engine = TickEngine::new();
    engine.register_types(&telegram_types(), &[]).unwrap();
    engine.register_types(&ha_types(), &[]).unwrap();
    engine
}
