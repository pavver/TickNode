//! SDK for TickNode node plugins.
//!
//! A plugin is a WASM component (`crate-type = ["cdylib"]`, target `wasm32-wasip2`) implementing
//! the `wit/plugin.wit` contract. Implement [`Plugin`] and [`PluginNode`], then invoke [`export_plugin!`].
//! Each graph node is a separate [`PluginNode`] instance, so state can be held directly in struct fields.
//!
//! # Language rules
//!
//! Definitions (titles, descriptions, help, pin names, type and setting texts) are written in
//! **English**: they are the fallback shown when no translation exists. Translations are shipped as
//! Fluent `.ftl` files via [`Plugin::TRANSLATIONS`]. **Logs are never translated**: everything passed
//! to [`host::log`] / [`host::warn`] / [`host::error`] and every node error message must be English.
//! Errors shown to the user in the UI (for example from [`Plugin::configure`]) are [`UiError`]s
//! with a translation key and an English fallback.
//!
//! # Translation key conventions
//!
//! Fluent ids may contain only `[a-zA-Z0-9_-]`, so names are slugified: every character outside
//! `[a-z0-9-]` (`/`, `.`, `_`, spaces) becomes `-` and letters are lowercased ([`slug`]).
//!
//! | what | key |
//! |------|-----|
//! | node title / description / help | `node-<type slug>-title`, `-description`, `-help` |
//! | property title / description | `node-<type slug>-prop-<name>-title`, `-description` (the property name as written when it consists of `[A-Za-z0-9_-]`, otherwise its slug) |
//! | select option label | `node-<type slug>-prop-<name>-option-<value slug>` |
//! | pin label | `node-<type slug>-pin-<pin name>`: the pin name as written when it consists of `[A-Za-z0-9_-]` (`pin-Val`), otherwise its slug (`Delay ms` -> `pin-delay-ms`) |
//! | category | `category-<category slug>` (the first plugin that defines a category key wins) |
//! | data type | `type-<type name slug>-title`, `-description`, `-field-<field>`, `-value-<enum value>` |
//! | setting | `setting-<plugin slug>-<key slug>-title`, `-description`, `-option-<value>` |
//! | free keys, e.g. `configure` errors | `<plugin slug>-...` (recommended: `<plugin slug>-error-...`; arguments are strings) |
//!
//! The host parses every `.ftl`; a resource with syntax errors is rejected (with a warning in the server
//! log) and messages whose ids do not start with one of the plugin's own prefixes are dropped, so
//! plugins cannot override each other's or the core's texts. Example (`include_str!` embeds the files):
//!
//! ```ignore
//! impl Plugin for MyPlugin {
//!     const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[
//!         ("uk", include_str!("../locales/uk.ftl")),
//!     ];
//! }
//! ```
//!
//! ```ignore
//! use ticknode_plugin_sdk::*;
//!
//! struct MyPlugin;
//! impl Plugin for MyPlugin {
//!     const NODE_TYPES: &'static [&'static str] = &["my/node"];
//!     fn manifest() -> PluginManifest { /* ... */ }
//!     fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> { /* ... */ }
//! }
//! export_plugin!(MyPlugin);
//! ```
//!
//! # Plugin-level connections and events
//!
//! A plugin can own persistent WebSocket connections (capability [`Capability::WebSocket`]):
//! [`host::ws_connect`] / [`host::ws_send`] / [`host::ws_close`] work from any plugin call
//! ([`Plugin::configure`], [`Plugin::tick`], node `execute`/`poll`). The host calls [`Plugin::tick`]
//! once per tick with the connection events ([`WsEvent`]); the plugin answers with [`Emission`]s: outputs
//! pushed to any of its own nodes, as if that node had executed. A node learns its graph id in
//! [`Plugin::create`], so the plugin keeps a `NodeId -> state` registry (in a `thread_local!`) to address
//! emissions. Time can be measured with `std::time::Instant` (WASI clocks).

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use std::collections::BTreeMap;

pub use ticknode_shared::{
    Capability, DataType, DataValue, EnumValue, FieldDefinition, NodeDefinition, NumberValue, ObjectField,
    NodeId, PinDefinition, PinDirection, PinId, PinType, PluginManifest, PropertyDefinition, PropertyKind, PropertyOption, SettingDefinition, SettingKind, TypeDefinition, TypeKind,
    UiMessage, fluent_slug as slug,
};

/// A translatable error for the user: `UiError::new("my-plugin-error-no-token", "Token is not set")`
/// (key in the plugin's translations, English fallback), optionally with `.arg("name", value)`.
/// `String` / `&str` convert into a fallback-only error.
pub type UiError = UiMessage;

#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        path: "../wit",
        world: "plugin",
        pub_export_macro: true,
        export_macro_name: "__export_ticknode_plugin",
        default_bindings_module: "ticknode_plugin_sdk::bindings",
    });
}

use bindings::exports::ticknode::plugin::nodes as wit_nodes;
use bindings::ticknode::plugin::{host as wit_host, types as wit};

pub type TaskId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// 0 — default host timeout.
    pub timeout_ms: u32,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self { method: "GET".into(), url: url.into(), headers: Vec::new(), body: Vec::new(), timeout_ms: 0 }
    }

    pub fn post(url: impl Into<String>, body: impl Into<Vec<u8>>) -> Self {
        Self { method: "POST".into(), body: body.into(), ..Self::get(url) }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOutput {
    Delay,
    Http(HttpResponse),
}

/// Reason why a node is executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Pulse on an input exec pin (or manual trigger from the editor).
    Pin(PinId),
    /// A task started by this node has completed.
    Task { id: TaskId, result: Result<TaskOutput, String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Pulse from an output exec pin.
    Exec(PinId),
    /// Value on an output data pin.
    Data(PinId, DataValue),
}

/// Handle of a plugin-owned WebSocket connection (see [`host::ws_connect`]). Void after the plugin
/// instance is recreated following a crash.
pub type WsId = u64;

/// Connection event delivered to [`Plugin::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    /// The connection is established.
    Opened(WsId),
    /// A text frame was received (binary frames are never delivered).
    Message(WsId, String),
    /// The connection ended or could not be opened; carries the reason.
    Closed(WsId, String),
}

/// Outputs for one node produced outside of its own `execute`/`poll`: applied by the host as if the
/// node had executed (exec pulses queue downstream nodes for the next tick, data goes to the node's outputs).
/// Emissions for foreign or unknown nodes and for invalid pins or values are dropped with an error log.
#[derive(Debug, Clone, PartialEq)]
pub struct Emission {
    pub node: NodeId,
    pub outputs: Vec<Output>,
}

impl Emission {
    pub fn new(node: NodeId, outputs: Vec<Output>) -> Self {
        Self { node, outputs }
    }
}

fn node_id_from_wit((hi, lo): (u64, u64)) -> NodeId {
    NodeId(uuid_from_halves(hi, lo))
}

fn node_id_to_wit(id: NodeId) -> (u64, u64) {
    id.0.as_u64_pair()
}

fn uuid_from_halves(hi: u64, lo: u64) -> ticknode_shared::Uuid {
    ticknode_shared::Uuid::from_u64_pair(hi, lo)
}

/// Values of connected input data pins.
#[derive(Debug, Default, Clone)]
pub struct Inputs(HashMap<PinId, DataValue>);

impl Inputs {
    pub fn get(&self, pin: PinId) -> Option<&DataValue> {
        self.0.get(&pin)
    }

    pub fn number(&self, pin: PinId, default: f64) -> f64 {
        match self.get(pin) {
            Some(DataValue::Number(n)) => n.to_f64(),
            _ => default,
        }
    }

    /// Field of a struct value on input `pin`.
    pub fn field(&self, pin: PinId, name: &str) -> Option<&DataValue> {
        field(self.get(pin)?, name)
    }

    pub fn field_number(&self, pin: PinId, name: &str) -> Option<f64> {
        field_number(self.get(pin)?, name)
    }

    pub fn field_string(&self, pin: PinId, name: &str) -> Option<&str> {
        field_string(self.get(pin)?, name)
    }

    pub fn field_bool(&self, pin: PinId, name: &str) -> Option<bool> {
        field_bool(self.get(pin)?, name)
    }

    pub fn bool(&self, pin: PinId, default: bool) -> bool {
        match self.get(pin) {
            Some(DataValue::Bool(b)) => *b,
            _ => default,
        }
    }

    /// String value, or an empty string if the pin is unconnected or holds another type.
    pub fn string(&self, pin: PinId) -> String {
        match self.get(pin) {
            Some(DataValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

/// Builder for a custom struct value: `StructValue::new("ha.state").field("state", "on").build()`.
/// Fields absent from `TypeDefinition` are not checked by the host, so field names must match the declaration.
#[derive(Debug, Clone)]
pub struct StructValue {
    type_name: String,
    fields: BTreeMap<String, ObjectField>,
}

impl StructValue {
    pub fn new(type_name: &str) -> Self {
        Self { type_name: type_name.into(), fields: BTreeMap::new() }
    }

    pub fn field(mut self, name: &str, value: impl Into<DataValue>) -> Self {
        self.fields.insert(name.into(), ObjectField { name: name.into(), description: String::new(), value: value.into() });
        self
    }

    /// Adds an optional field only if `value` is `Some`.
    pub fn field_opt(self, name: &str, value: Option<impl Into<DataValue>>) -> Self {
        match value {
            Some(v) => self.field(name, v),
            None => self,
        }
    }

    pub fn build(self) -> DataValue {
        DataValue::Object { fields: self.fields, type_name: Some(self.type_name) }
    }
}

impl From<StructValue> for DataValue {
    fn from(v: StructValue) -> Self { v.build() }
}

/// Field of a struct value (or any object).
pub fn field<'a>(value: &'a DataValue, name: &str) -> Option<&'a DataValue> {
    match value {
        DataValue::Object { fields, .. } => fields.get(name).map(|f| &f.value),
        _ => None,
    }
}

pub fn field_number(value: &DataValue, name: &str) -> Option<f64> {
    match field(value, name)? {
        DataValue::Number(n) => Some(n.to_f64()),
        _ => None,
    }
}

pub fn field_string<'a>(value: &'a DataValue, name: &str) -> Option<&'a str> {
    match field(value, name)? {
        DataValue::String(s) => Some(s),
        _ => None,
    }
}

pub fn field_bool(value: &DataValue, name: &str) -> Option<bool> {
    match field(value, name)? {
        DataValue::Bool(b) => Some(*b),
        _ => None,
    }
}

/// A plugin node. One instance corresponds to one node in the graph.
pub trait PluginNode: 'static {
    /// Pins may depend on properties.
    fn definition(&self) -> NodeDefinition;
    /// Properties used to reconstruct the node (graph persistence, plugin restart).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    fn set_property(&mut self, _name: &str, _value: DataValue) {}
    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output>;
    /// For event source nodes: called at the start of every tick when `polls()` is `true`.
    fn poll(&mut self, _tick: u64) -> Option<PinId> { None }
    /// Whether this node is an event source and the host should call `poll` every tick.
    fn polls(&self) -> bool { false }
}

pub trait Plugin: 'static {
    /// All node types provided by the plugin; palette definitions come from `create(type).definition()`.
    const NODE_TYPES: &'static [&'static str];
    fn manifest() -> PluginManifest;
    /// Creates a node; `node` is its graph id (the same id again after the node is recreated following a
    /// crash). Definitions are read from nodes created with the nil id.
    fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>>;
    /// Custom plugin data types (structs and enums); node pins can reference them
    /// via `DataType::Named`. The host automatically generates Make/Break nodes for each struct.
    fn types() -> Vec<TypeDefinition> { Vec::new() }
    /// Translations shipped by the plugin: `(locale code, Fluent source)` pairs, usually
    /// `include_str!("../locales/uk.ftl")`. See the crate docs for key conventions and validation.
    /// The English texts live in the definitions, so an `en` resource is optional.
    const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[];
    /// Applies settings according to `manifest().settings` (values are saved or defaults).
    /// Called by the host after loading, on every change, and after recreating the instance
    /// (crash). There is one instance per plugin, so settings are typically stored in static state
    /// (`thread_local!`). `Err` is shown to the user (translated through its key; the key must start
    /// with the plugin name slug) while the plugin remains loaded. Texts logged here stay English.
    fn configure(_settings: &[(String, DataValue)]) -> Result<(), UiError> { Ok(()) }
    /// Called once per tick for the whole plugin, even without events (so it can drive timers and
    /// keep-alives), after finished tasks are delivered and before event sources are polled. `events` are
    /// the WebSocket events since the previous tick (the host delivers at most 1000 per tick). The returned
    /// emissions are applied to the plugin's nodes. Runs under the per-call fuel limit.
    fn tick(_events: Vec<WsEvent>) -> Vec<Emission> { Vec::new() }
}

/// Looks up a setting value by key.
pub fn setting<'a>(settings: &'a [(String, DataValue)], key: &str) -> Option<&'a DataValue> {
    settings.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// String setting (an empty string is treated as unset).
pub fn setting_str<'a>(settings: &'a [(String, DataValue)], key: &str) -> Option<&'a str> {
    match setting(settings, key)? {
        DataValue::String(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

pub fn setting_number(settings: &[(String, DataValue)], key: &str) -> Option<f64> {
    match setting(settings, key)? {
        DataValue::Number(n) => Some(n.to_f64()),
        _ => None,
    }
}

pub fn setting_bool(settings: &[(String, DataValue)], key: &str) -> Option<bool> {
    match setting(settings, key)? {
        DataValue::Bool(b) => Some(*b),
        _ => None,
    }
}

/// Host services. Tasks (`delay_*`, `http`) can only be started during `execute`/`poll`;
/// their result arrives in `execute` as [`Trigger::Task`].
pub mod host {
    use super::*;

    /// Logs an info message to the node's console. Logs are always English and never translated.
    pub fn log(message: &str) {
        wit_host::log(wit::LogLevel::Info, message);
    }

    /// Logs a warning (English, like every log).
    pub fn warn(message: &str) {
        wit_host::log(wit::LogLevel::Warn, message);
    }

    /// Logs an error (English, like every log). Use [`UiError`] for errors the user must see in the UI.
    pub fn error(message: &str) {
        wit_host::log(wit::LogLevel::Error, message);
    }

    pub fn current_tick() -> u64 {
        wit_host::current_tick()
    }

    pub fn tick_duration_ms() -> u32 {
        wit_host::tick_duration_ms()
    }

    pub fn delay_ticks(ticks: u64) -> Result<TaskId, String> {
        wit_host::delay(ticks)
    }

    /// Delay rounded up to whole ticks.
    pub fn delay_ms(ms: u64) -> Result<TaskId, String> {
        let tick = u64::from(tick_duration_ms().max(1));
        delay_ticks(ms.div_ceil(tick))
    }

    pub fn http(request: HttpRequest) -> Result<TaskId, String> {
        wit_host::start_http(&wit::HttpRequest {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
            timeout_ms: request.timeout_ms,
        })
    }

    /// Opens a plugin-owned WebSocket (`ws://` or `wss://`; requires [`Capability::WebSocket`] for the host).
    /// Returns at once: [`WsEvent::Opened`] (or [`WsEvent::Closed`] if connecting failed) arrives in
    /// [`Plugin::tick`]. At most 4 connections per plugin.
    pub fn ws_connect(url: &str, headers: &[(String, String)]) -> Result<WsId, String> {
        wit_host::ws_connect(url, headers)
    }

    /// Sends a text frame. Fails if the connection is unknown or already closed.
    pub fn ws_send(id: WsId, text: &str) -> Result<(), String> {
        wit_host::ws_send(id, text)
    }

    /// Closes a connection; no [`WsEvent::Closed`] is delivered for it afterwards.
    pub fn ws_close(id: WsId) {
        wit_host::ws_close(id)
    }

    /// Publishes the options of a dynamic property (`PropertyDefinition::dynamic(.., source)`), replacing the
    /// previous list of that source. Usable from any plugin call. The host drops over-long strings and caps the
    /// list at 20 000 options; an empty list removes the source; an unchanged list is not re-sent to editors.
    pub fn set_options(source: &str, options: &[PropertyOption]) {
        let options: Vec<wit::PropertyOption> = options.iter().map(|o| wit::PropertyOption {
            value: o.value.clone(),
            label: o.label.clone(),
            detail: o.detail.clone(),
            icon: o.icon.clone(),
        }).collect();
        wit_host::set_options(source, &options);
    }

    /// Cancels a task belonging to this node. Returns `false` if the task has already finished.
    pub fn cancel(task: TaskId) -> bool {
        wit_host::cancel_task(task)
    }
}

/// Exports the plugin as a WASM component. Invoked once in a plugin crate.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        type __TickNodeExport = $crate::Exporter<$plugin>;
        $crate::bindings::__export_ticknode_plugin!(__TickNodeExport with_types_in $crate::bindings);
    };
}

// --- WIT export implementation ---

#[doc(hidden)]
pub struct Exporter<P>(PhantomData<P>);

#[doc(hidden)]
pub struct NodeHandle(RefCell<Box<dyn PluginNode>>);

impl<P: Plugin> wit_nodes::Guest for Exporter<P> {
    type Node = NodeHandle;

    fn manifest() -> wit::PluginManifest {
        let m = P::manifest();
        wit::PluginManifest {
            name: m.name,
            version: m.version,
            author: m.author,
            capabilities: m.requested_capabilities.into_iter().map(|c| match c {
                Capability::Http { allowed_domains } => wit::Capability::Http(allowed_domains),
                Capability::Storage => wit::Capability::Storage,
                Capability::WebSocket { allowed_hosts } => wit::Capability::Websocket(allowed_hosts),
            }).collect(),
            settings: m.settings.into_iter().map(to_wit_setting).collect(),
        }
    }

    fn configure(settings: Vec<wit::Property>) -> Result<(), wit::UiError> {
        let settings: Vec<(String, DataValue)> = settings.into_iter().filter_map(|p| Some((p.name, decode(&p.value)?))).collect();
        P::configure(&settings).map_err(|e| wit::UiError { key: e.key, args: e.args, fallback: e.fallback })
    }

    fn translations() -> Vec<(String, String)> {
        P::TRANSLATIONS.iter().map(|(locale, ftl)| ((*locale).to_string(), (*ftl).to_string())).collect()
    }

    fn definitions() -> Vec<wit::NodeDefinition> {
        P::NODE_TYPES.iter().filter_map(|t| P::create(t, NodeId(ticknode_shared::Uuid::nil()))).map(|n| to_wit_definition(n.definition(), n.polls())).collect()
    }

    fn types() -> Vec<wit::TypeDefinition> {
        P::types().into_iter().map(to_wit_type_definition).collect()
    }

    fn create_node(type_name: String, node: wit::NodeId) -> Result<wit_nodes::Node, String> {
        P::create(&type_name, node_id_from_wit(node))
            .map(|node| wit_nodes::Node::new(NodeHandle(RefCell::new(node))))
            .ok_or_else(|| format!("Unknown node type: {type_name}"))
    }

    fn plugin_tick(events: Vec<wit::WsEvent>) -> Vec<wit::Emission> {
        let events = events.into_iter().map(|e| match e {
            wit::WsEvent::Opened(id) => WsEvent::Opened(id),
            wit::WsEvent::Message((id, text)) => WsEvent::Message(id, text),
            wit::WsEvent::Closed((id, reason)) => WsEvent::Closed(id, reason),
        }).collect();
        P::tick(events).into_iter().map(|e| wit::Emission {
            node: node_id_to_wit(e.node),
            outputs: e.outputs.into_iter().map(|o| match o {
                Output::Exec(pin) => wit::Output::Exec(pin.0),
                Output::Data(pin, value) => wit::Output::Data((pin.0, encode(&value))),
            }).collect(),
        }).collect()
    }
}

impl wit_nodes::GuestNode for NodeHandle {
    fn definition(&self) -> wit::NodeDefinition {
        let node = self.0.borrow();
        to_wit_definition(node.definition(), node.polls())
    }

    fn properties(&self) -> Vec<wit::Property> {
        self.0.borrow().properties().into_iter()
            .map(|(name, value)| wit::Property { name, value: encode(&value) })
            .collect()
    }

    fn set_property(&self, name: String, value: wit::Value) {
        if let Some(value) = decode(&value) {
            self.0.borrow_mut().set_property(&name, value);
        }
    }

    fn execute(&self, trigger: wit::Trigger, inputs: Vec<(u32, wit::Value)>) -> Vec<wit::Output> {
        let trigger = match trigger {
            wit::Trigger::Pin(pin) => Trigger::Pin(PinId(pin)),
            wit::Trigger::Task(done) => Trigger::Task {
                id: done.id,
                result: done.result.map(|output| match output {
                    wit::TaskOutput::Delay => TaskOutput::Delay,
                    wit::TaskOutput::Http(r) => TaskOutput::Http(HttpResponse { status: r.status, headers: r.headers, body: r.body }),
                }),
            },
        };
        let inputs = Inputs(inputs.into_iter().filter_map(|(pin, v)| Some((PinId(pin), decode(&v)?))).collect());
        self.0.borrow_mut().execute(trigger, &inputs).into_iter().map(|o| match o {
            Output::Exec(pin) => wit::Output::Exec(pin.0),
            Output::Data(pin, value) => wit::Output::Data((pin.0, encode(&value))),
        }).collect()
    }

    fn poll(&self, tick: u64) -> Option<u32> {
        self.0.borrow_mut().poll(tick).map(|p| p.0)
    }
}

fn encode(value: &DataValue) -> Vec<u8> {
    postcard::to_allocvec(value).expect("DataValue always serializes")
}

fn decode(bytes: &[u8]) -> Option<DataValue> {
    postcard::from_bytes(bytes).ok()
}

fn to_wit_data_type(mut t: &DataType) -> wit::DataType {
    let mut list_depth = 0u8;
    while let DataType::List(inner) = t {
        list_depth += 1;
        t = inner;
    }
    let base = match t {
        DataType::Bool => wit::ScalarType::Bool,
        DataType::Number => wit::ScalarType::Number,
        DataType::String => wit::ScalarType::String,
        DataType::Time => wit::ScalarType::Time,
        DataType::DateTime => wit::ScalarType::Datetime,
        DataType::Duration => wit::ScalarType::Duration,
        DataType::Color => wit::ScalarType::Color,
        DataType::Object => wit::ScalarType::Object,
        DataType::Any => wit::ScalarType::Any,
        DataType::Named(name) => wit::ScalarType::Named(name.clone()),
        DataType::List(_) => unreachable!("lists are unwrapped above"),
    };
    wit::DataType { base, list_depth }
}

fn to_wit_setting(s: SettingDefinition) -> wit::SettingDefinition {
    wit::SettingDefinition {
        key: s.key,
        title: s.title,
        description: s.description,
        kind: match s.kind {
            SettingKind::Text => wit::SettingKind::Text,
            SettingKind::Number => wit::SettingKind::Number,
            SettingKind::Bool => wit::SettingKind::Bool,
            SettingKind::Secret => wit::SettingKind::Secret,
            SettingKind::Url => wit::SettingKind::Url,
            SettingKind::Select(options) => wit::SettingKind::Select(options),
        },
        required: s.required,
        default: s.default.as_ref().map(encode),
    }
}

fn to_wit_type_definition(t: TypeDefinition) -> wit::TypeDefinition {
    wit::TypeDefinition {
        name: t.name,
        title: t.title,
        description: t.description,
        color: t.color,
        kind: match t.kind {
            TypeKind::Struct { fields } => wit::TypeKind::Structure(fields.into_iter().map(|f| wit::FieldDefinition {
                name: f.name,
                title: f.title,
                description: f.description,
                data_type: to_wit_data_type(&f.data_type),
                optional: f.optional,
            }).collect()),
            TypeKind::Enum { values } => wit::TypeKind::Enumeration(
                values.into_iter().map(|v| wit::EnumValue { value: v.value, title: v.title }).collect(),
            ),
        },
    }
}

fn to_wit_definition(d: NodeDefinition, polls: bool) -> wit::NodeDefinition {
    wit::NodeDefinition {
        type_name: d.type_name,
        title: d.title,
        category: d.category,
        description: d.description,
        help: d.help,
        pins: d.pins.into_iter().map(|p| wit::Pin {
            id: p.id.0,
            required: p.required,
            name: p.name,
            pin_type: match &p.pin_type {
                PinType::Exec => wit::PinType::Exec,
                PinType::Data(t) => wit::PinType::Data(to_wit_data_type(t)),
            },
            direction: match p.direction {
                PinDirection::Input => wit::PinDirection::Input,
                PinDirection::Output => wit::PinDirection::Output,
            },
        }).collect(),
        polls,
        properties: d.properties.into_iter().map(|p| wit::PropertyDefinition {
            name: p.name,
            title: p.title,
            description: p.description,
            kind: match p.kind {
                PropertyKind::Text => wit::PropertyKind::Text,
                PropertyKind::Select(options) => wit::PropertyKind::Select(options),
                PropertyKind::Dynamic { source, .. } => wit::PropertyKind::Dynamic(source),
            },
        }).collect(),
    }
}

/// Shorthand helpers for defining pins in `definition()`.
pub fn pin(id: u32, name: &str, pin_type: PinType, direction: PinDirection) -> PinDefinition {
    PinDefinition { id: PinId(id), name: name.to_string(), pin_type, direction, required: false }
}

pub fn exec_in(id: u32, name: &str) -> PinDefinition {
    pin(id, name, PinType::Exec, PinDirection::Input)
}

pub fn exec_out(id: u32, name: &str) -> PinDefinition {
    pin(id, name, PinType::Exec, PinDirection::Output)
}

pub fn data_in(id: u32, name: &str, t: DataType) -> PinDefinition {
    pin(id, name, PinType::Data(t), PinDirection::Input)
}

pub fn data_out(id: u32, name: &str, t: DataType) -> PinDefinition {
    pin(id, name, PinType::Data(t), PinDirection::Output)
}
