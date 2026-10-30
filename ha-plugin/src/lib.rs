//! Home Assistant integration.
//!
//! One WebSocket connection per plugin, driven from [`Plugin::tick`] by the sans-IO `Session` of
//! `ha-core-client` (see [`hub`]). Nodes are thin: `ha/state_changed` is an event source fed by emissions,
//! `ha/get_state` and `ha/call_service` start a request whose answer arrives in a later tick, also as an emission.

mod convert;
mod hub;
mod queue;

use std::cell::RefCell;
use std::sync::Once;
use std::time::Instant;

use ticknode_plugin_sdk::*;

use convert::{STATE_TYPE, value_to_json};
use hub::{CALL_DONE, CALL_ERROR, CALL_FAILED, CHANGED, Env, GET_DONE, GET_ERROR, GET_FAILED, GET_STATE, Hub, NEW, OLD};
use queue::Mode;

/// The endpoint of the Home Assistant add-on environment (the supervisor proxies the core).
const DEFAULT_URL: &str = "ws://supervisor/core/websocket";
const CATEGORY: &str = "Home Assistant";

thread_local! {
    /// There is one plugin instance, so the connection and the subscriptions live in static state.
    static HUB: RefCell<Hub> = RefCell::new(Hub::default());
}

fn with_hub<R>(f: impl FnOnce(&mut Hub, &mut HostEnv) -> R) -> R {
    HUB.with(|hub| f(&mut hub.borrow_mut(), &mut HostEnv))
}

/// Like [`with_hub`], but silent if the hub is busy or gone (used from `Drop`).
fn try_with_hub(f: impl FnOnce(&mut Hub, &mut HostEnv)) {
    let _ = HUB.try_with(|hub| {
        if let Ok(mut hub) = hub.try_borrow_mut() {
            f(&mut hub, &mut HostEnv);
        }
    });
}

struct HostEnv;

impl Env for HostEnv {
    fn ws_connect(&mut self, url: &str) -> Result<WsId, String> {
        host::ws_connect(url, &[])
    }

    fn ws_send(&mut self, id: WsId, text: &str) -> Result<(), String> {
        host::ws_send(id, text)
    }

    fn ws_close(&mut self, id: WsId) {
        host::ws_close(id)
    }

    fn info(&mut self, message: &str) {
        host::log(message)
    }

    fn warn(&mut self, message: &str) {
        host::warn(message)
    }

    fn error(&mut self, message: &str) {
        host::error(message)
    }
}

/// Forwards warnings and errors of `ha-core-client` (its `log` facade) to the host log.
struct CoreLogger;

impl log::Log for CoreLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record) {
        let message = format!("ha-core-client: {}", record.args());
        match record.level() {
            log::Level::Error => host::error(&message),
            _ => host::warn(&message),
        }
    }

    fn flush(&self) {}
}

fn install_core_logger() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if log::set_logger(&CoreLogger).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
    });
}

fn text_property(value: DataValue) -> Option<String> {
    match value {
        DataValue::String(s) => Some(s.trim().to_string()),
        _ => None,
    }
}

// --- ha/state_changed ---

struct StateChanged {
    node: NodeId,
    /// `None` for the nil-id node used to read the definition: it never touches the hub.
    serial: Option<u64>,
    entity: String,
    mode: Mode,
}

impl StateChanged {
    fn new(node: NodeId) -> Self {
        let mode = Mode::default();
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_watch(node, mode)));
        Self { node, serial, entity: String::new(), mode }
    }
}

impl Drop for StateChanged {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_watch(self.node, serial, Instant::now(), env));
        }
    }
}

impl PluginNode for StateChanged {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "ha/state_changed".into(),
            title: "HA State Changed".into(),
            category: CATEGORY.into(),
            description: "Fires when a Home Assistant entity changes".into(),
            help: "Event source: sends a signal on Changed every time the entity from the entity_id property changes its state or attributes in Home Assistant. \
New is the state after the change and Old the state before it. If the entity did not exist before (or was removed), that state has an empty state field.\n\n\
The mode property says what happens when several changes arrive in the same tick: \"every\" (default) sends one signal per change, in order, one per tick; \"latest\" sends only the newest change of the tick.\n\n\
Needs the Home Assistant connection to be set up in the plugin settings. While the connection is down no changes are seen, and after it returns a change that happened meanwhile is delivered as one signal. \
A nonexistent entity is retried every 30 seconds.".into(),
            pins: vec![
                exec_out(CHANGED.0, "Changed"),
                data_out(NEW.0, "New", DataType::Named(STATE_TYPE.into())),
                data_out(OLD.0, "Old", DataType::Named(STATE_TYPE.into())),
            ],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("entity_id".into(), DataValue::String(self.entity.clone())),
            ("mode".into(), DataValue::String(self.mode.as_str().into())),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else { return };
        match name {
            "entity_id" => {
                if let Some(serial) = self.serial {
                    with_hub(|hub, env| hub.set_entity(self.node, serial, &text, Instant::now(), env));
                }
                self.entity = text;
            }
            "mode" => match Mode::parse(&text) {
                Some(mode) => {
                    self.mode = mode;
                    if let Some(serial) = self.serial {
                        with_hub(|hub, _| hub.set_mode(self.node, serial, mode));
                    }
                }
                None => host::warn(&format!("Unknown mode \"{text}\" for ha/state_changed (expected \"every\" or \"latest\"): keeping \"{}\"", self.mode.as_str())),
            },
            _ => {}
        }
    }

    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
}

// --- ha/get_state ---

struct GetState {
    node: NodeId,
}

impl PluginNode for GetState {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "ha/get_state".into(),
            title: "HA Get State".into(),
            category: CATEGORY.into(),
            description: "Reads the current state of a Home Assistant entity".into(),
            help: "On a signal at In asks Home Assistant for the current state of Entity ID. The answer arrives a little later (a few ticks): \
on success State holds the entity and a signal goes to Done; on failure (not connected, no such entity, timeout) Error holds the reason and a signal goes to Failed. \
After a failure State is an empty state (its state field is empty).\n\nThe request fails at once if the connection to Home Assistant is down.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Entity ID", DataType::String).required(),
                exec_out(GET_DONE.0, "Done"),
                exec_out(GET_FAILED.0, "Failed"),
                data_out(GET_STATE.0, "State", DataType::Named(STATE_TYPE.into())),
                data_out(GET_ERROR.0, "Error", DataType::String),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let entity = inputs.string(PinId(1)).trim().to_string();
        with_hub(|hub, env| hub.get_state(self.node, &entity, Instant::now(), env)).unwrap_or_default()
    }
}

// --- ha/call_service ---

struct CallService {
    node: NodeId,
}

impl PluginNode for CallService {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "ha/call_service".into(),
            title: "HA Call Service".into(),
            category: CATEGORY.into(),
            description: "Calls a Home Assistant service".into(),
            help: "On a signal at In calls the Home Assistant service Domain.Service (for example light + turn_on). \
Entity ID is the optional target entity; Data is an optional object with the service data (for example {brightness: 120}).\n\n\
When Home Assistant confirms the call a signal goes to Done; otherwise (not connected, rejected, timeout) Error holds the reason and a signal goes to Failed. \
The answer arrives a little later (a few ticks); the request fails at once if the connection is down.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Domain", DataType::String).required(),
                data_in(2, "Service", DataType::String).required(),
                data_in(3, "Entity ID", DataType::String),
                data_in(4, "Data", DataType::Object),
                exec_out(CALL_DONE.0, "Done"),
                exec_out(CALL_FAILED.0, "Failed"),
                data_out(CALL_ERROR.0, "Error", DataType::String),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let domain = inputs.string(PinId(1)).trim().to_string();
        let service = inputs.string(PinId(2)).trim().to_string();
        let entity = inputs.string(PinId(3)).trim().to_string();
        let data = inputs.get(PinId(4)).map_or(serde_json::Value::Null, value_to_json);
        with_hub(|hub, env| hub.call_service(self.node, &domain, &service, &entity, data, Instant::now(), env)).unwrap_or_default()
    }
}

// --- The plugin ---

struct HomeAssistant;

impl Plugin for HomeAssistant {
    const NODE_TYPES: &'static [&'static str] = &["ha/call_service", "ha/get_state", "ha/state_changed"];
    const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[("uk", include_str!("../locales/uk.ftl"))];

    fn manifest() -> PluginManifest {
        PluginManifest::new("home_assistant", "0.1.0", "TickNode")
            .capability(Capability::WebSocket { allowed_hosts: vec!["*".into()] })
            .setting(
                SettingDefinition::url("url", "URL")
                    .described("WebSocket address of Home Assistant. The default is the add-on endpoint; elsewhere use ws://<host>:8123/api/websocket")
                    .default_value(DEFAULT_URL),
            )
            .setting(
                SettingDefinition::secret("token", "Access token")
                    .described("A long-lived access token (Profile > Security in Home Assistant), or the supervisor token in the add-on")
                    .required(),
            )
    }

    fn types() -> Vec<TypeDefinition> {
        vec![
            TypeDefinition::new_struct(STATE_TYPE, "HA State", vec![
                FieldDefinition::new("entity_id", "Entity ID", DataType::String),
                FieldDefinition::new("state", "State", DataType::String).described("The state text; empty if the entity does not exist"),
                FieldDefinition::new("attributes", "Attributes", DataType::Object),
                FieldDefinition::new("last_changed", "Last changed", DataType::DateTime),
                FieldDefinition::new("last_updated", "Last updated", DataType::DateTime),
            ])
            .described("The state of a Home Assistant entity")
            .colored("#18bcf2"),
        ]
    }

    fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> {
        Some(match type_name {
            "ha/state_changed" => Box::new(StateChanged::new(node)),
            "ha/get_state" => Box::new(GetState { node }),
            "ha/call_service" => Box::new(CallService { node }),
            _ => return None,
        })
    }

    fn configure(settings: &[(String, DataValue)]) -> Result<(), UiError> {
        install_core_logger();
        let url = setting_str(settings, "url").unwrap_or(DEFAULT_URL).trim().to_string();
        let token = setting_str(settings, "token").map(str::trim).unwrap_or_default().to_string();
        let now = Instant::now();
        // The old connection is closed in any case; a valid configuration opens a new one on the next tick.
        with_hub(|hub, env| hub.reset(now, env));
        if token.is_empty() {
            return Err(UiError::new("home-assistant-error-no-token", "The access token is not set"));
        }
        if !(url.starts_with("ws://") || url.starts_with("wss://")) {
            return Err(UiError::new("home-assistant-error-bad-url", format!("The URL must start with ws:// or wss://, got \"{url}\"")).arg("url", url));
        }
        with_hub(|hub, env| hub.configure(&url, &token, now, env));
        Ok(())
    }

    fn tick(events: Vec<WsEvent>) -> Vec<Emission> {
        with_hub(|hub, env| hub.tick(Instant::now(), events, env))
    }
}

export_plugin!(HomeAssistant);
