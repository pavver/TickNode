//! Telegram integration. Transport, storage and time are injected into the sans-IO hub.
mod actions;
mod api;
mod directory;
mod hub;
mod model;
mod runtime;
mod schema;
mod triggers;
mod validation;

use ticknode_plugin_sdk::*;

pub struct Telegram;
impl Plugin for Telegram {
    const NODE_TYPES: &'static [&'static str] = &[
        "telegram/message",
        "telegram/command",
        "telegram/callback",
        "telegram/send",
        "telegram/edit",
        "telegram/delete",
    ];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[
        ("en", include_str!("../locales/en.ftl")),
        ("uk", include_str!("../locales/uk.ftl")),
    ];
    fn manifest() -> PluginManifest {
        PluginManifest::new("telegram", "0.1.0", "TickNode")
            .capability(Capability::Http { allowed_domains: vec!["api.telegram.org".into()] })
            .capability(Capability::Storage)
            .setting(SettingDefinition::secret("token", "Bot token")
                .described("Token from @BotFather; verified with getMe").required())
            .setting(SettingDefinition::text("allowed_chats", "Allowed chats")
                .described("Optional comma-separated chat IDs; restricts receiving and sending"))
            .setting(SettingDefinition::number("directory_ttl_days", "Directory retention (days)")
                .described("Expire unseen directory entries after this many days; 0 keeps them forever")
                .default_value(30i64))
    }
    fn types() -> Vec<TypeDefinition> {
        schema::types()
    }
    fn create(name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> {
        let kind = name.strip_prefix("telegram/")?;
        Some(match kind {
            "message" | "command" | "callback" => Box::new(triggers::TriggerNode::new(kind, node)),
            "send" | "edit" | "delete" => Box::new(actions::ActionNode::new(kind)),
            _ => return None,
        })
    }
    fn configure(settings: &[(String, DataValue)]) -> Result<(), UiError> {
        runtime::with(|hub, env| hub.configure(settings, runtime::now(), env))
    }
    fn tick(events: Vec<PluginEvent>) -> Vec<Emission> {
        runtime::with(|hub, env| hub.tick(runtime::now(), events, env))
    }
}

#[cfg(target_arch = "wasm32")]
export_plugin!(Telegram);
#[cfg(test)]
mod tests;
