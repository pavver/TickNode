//! Home Assistant integration.
//!
//! One WebSocket connection per plugin, driven from [`Plugin::tick`] by the sans-IO `Session` of
//! `ha-core-client` (see [`hub`]). Nodes are thin: `ha/state_changed` is an event source fed by emissions,
//! `ha/entity` exposes live data, `ha/action` builds a typed action form, `ha/trigger_*` expose automation
//! triggers, and `ha/condition_*` gate execution using cached states (or async Jinja). `ha/get_state`
//! and `ha/call_service` start a request whose answer arrives in a later tick, also as an emission.

mod actions;
mod conditions;
mod convert;
mod entities;
mod entity;
mod hub;
mod icons;
mod live;
mod options;
mod queue;
mod requests;
mod runtime;
mod state_changed;
mod states;
mod triggers;

use convert::STATE_TYPE;
use requests::{CallService, GetState};
use runtime::{install_core_logger, try_with_hub, with_hub};
use state_changed::StateChanged;
use std::time::Instant;
use ticknode_plugin_sdk::*;

/// The endpoint of the Home Assistant add-on environment (the supervisor proxies the core).
const DEFAULT_URL: &str = "ws://homeassistant.local:8123/api/websocket";
const CATEGORY: &str = "Home Assistant";

fn text_property(value: DataValue) -> Option<String> {
    match value {
        DataValue::String(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

// --- The plugin ---

struct HomeAssistant;

impl Plugin for HomeAssistant {
    const NODE_TYPES: &'static [&'static str] = &[
        "ha/action",
        "ha/entity",
        "ha/call_service",
        "ha/get_state",
        "ha/state_changed",
        "ha/trigger_state",
        "ha/trigger_numeric_state",
        "ha/trigger_time",
        "ha/trigger_time_pattern",
        "ha/trigger_sun",
        "ha/trigger_zone",
        "ha/trigger_event",
        "ha/trigger_homeassistant",
        "ha/trigger_mqtt",
        "ha/trigger_webhook",
        "ha/trigger_calendar",
        "ha/trigger_tag",
        "ha/trigger_conversation",
        "ha/trigger_template",
        "ha/condition_state",
        "ha/condition_numeric_state",
        "ha/condition_time",
        "ha/condition_sun",
        "ha/condition_zone",
        "ha/condition_template",
    ];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] =
        &[("uk", include_str!("../locales/uk.ftl"))];

    fn manifest() -> PluginManifest {
        PluginManifest::new("home_assistant", "0.1.0", "TickNode")
            .capability(Capability::WebSocket { allowed_hosts: vec!["*".into()] })
            .setting(
                SettingDefinition::url("url", "URL")
                    .described("WebSocket address of Home Assistant: ws://<host>:8123/api/websocket (wss:// with TLS). The add-on sets it to its Supervisor endpoint automatically")
                    .default_value(DEFAULT_URL),
            )
            .setting(
                SettingDefinition::secret("token", "Access token")
                    .described("A long-lived access token (Profile > Security in Home Assistant), or the supervisor token in the add-on")
                    .required(),
            )
            .resource(
                ResourceCollection::new("entities", "Entities", entities::ENTITIES_SOURCE)
                    .icon("mdi:home-assistant")
                    .action(ResourceAction::new("State trigger", "ha/trigger_state", "entity_id"))
                    .action(ResourceAction::new("Value", "ha/entity", "entity_id"))
                    .action(ResourceAction::new("State changed", "ha/state_changed", "entity_id"))
                    .action(ResourceAction::new("State condition", "ha/condition_state", "entity_id"))
                    .action(ResourceAction::new("Action", "ha/action", "target").requires_source(entities::ACTION_TARGETS_SOURCE)),
            )
            .resource(
                ResourceCollection::new("zones", "Zones", "zones")
                    .icon("mdi:map-marker-radius")
                    .action(ResourceAction::new("Zone trigger", "ha/trigger_zone", "zone"))
                    .action(ResourceAction::new("Zone condition", "ha/condition_zone", "zone")),
            )
            .resource(
                ResourceCollection::new("persons", "People", "persons")
                    .icon("mdi:account")
                    .action(ResourceAction::new("Zone condition", "ha/condition_zone", "entity_id")),
            )
    }

    fn types() -> Vec<TypeDefinition> {
        vec![
            TypeDefinition::new_struct(
                STATE_TYPE,
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
            .described("The state of a Home Assistant entity")
            .colored("#18bcf2"),
        ]
    }

    fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> {
        Some(match type_name {
            "ha/action" => Box::new(actions::ActionNode::new(node)),
            "ha/entity" => Box::new(entity::Entity::new(node)),
            "ha/state_changed" => Box::new(StateChanged::new(node)),
            "ha/get_state" => Box::new(GetState { node }),
            "ha/call_service" => Box::new(CallService { node }),
            _ => {
                if let Some(kind) = conditions::KINDS
                    .iter()
                    .find(|k| k.type_name() == type_name)
                {
                    Box::new(conditions::ConditionNode::new(*kind, node))
                } else {
                    let platform = triggers::PLATFORMS
                        .iter()
                        .find(|p| p.type_name() == type_name)?;
                    Box::new(triggers::TriggerNode::new(*platform, node))
                }
            }
        })
    }

    fn configure(settings: &[(String, DataValue)]) -> Result<(), UiError> {
        install_core_logger();
        let url = setting_str(settings, "url")
            .unwrap_or(DEFAULT_URL)
            .trim()
            .to_string();
        let token = setting_str(settings, "token")
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        let now = Instant::now();
        // The old connection is closed in any case; a valid configuration opens a new one on the next tick.
        with_hub(|hub, env| hub.reset(now, env));
        validate_connection(&url, &token)?;
        with_hub(|hub, env| hub.configure(&url, &token, now, env));
        Ok(())
    }

    fn tick(events: Vec<WsEvent>) -> Vec<Emission> {
        with_hub(|hub, env| hub.tick(Instant::now(), events, env))
    }
}

fn validate_connection(url: &str, token: &str) -> Result<(), UiError> {
    if token.is_empty() {
        return Err(UiError::new(
            "home-assistant-error-no-token",
            "The access token is not set",
        ));
    }
    if !(url.starts_with("ws://") || url.starts_with("wss://")) {
        return Err(UiError::new(
            "home-assistant-error-bad-url",
            format!("The URL must start with ws:// or wss://, got \"{url}\""),
        )
        .arg("url", url));
    }
    Ok(())
}

export_plugin!(HomeAssistant);

#[cfg(test)]
mod tests;
