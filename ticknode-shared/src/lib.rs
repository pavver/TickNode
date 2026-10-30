use serde::{Deserialize, Serialize};
pub use uuid::Uuid;
use chrono::{NaiveTime, DateTime, Utc};
use std::time::Duration;
use std::ops::{Add, Sub, Mul, Div};
use std::collections::BTreeMap;

pub mod condition;
pub mod text;
pub mod plugin;
pub mod types;
pub mod ui_message;

pub use plugin::*;
pub use types::*;
pub use ui_message::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub Uuid);

impl NodeId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId(pub u32);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum NumberValue {
    Int(i64),
    Float(f64),
}

impl NumberValue {
    pub fn to_f64(self) -> f64 {
        match self {
            Self::Int(v) => v as f64,
            Self::Float(v) => v,
        }
    }

    pub fn normalize(self) -> Self {
        match self {
            Self::Float(f) => {
                if f.fract() == 0.0 && f <= i64::MAX as f64 && f >= i64::MIN as f64 {
                    Self::Int(f as i64)
                } else {
                    Self::Float(f)
                }
            }
            Self::Int(i) => Self::Int(i),
        }
    }
}

impl Add for NumberValue {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a + b),
            (a, b) => Self::Float(a.to_f64() + b.to_f64()).normalize(),
        }
    }
}

impl Sub for NumberValue {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a - b),
            (a, b) => Self::Float(a.to_f64() - b.to_f64()).normalize(),
        }
    }
}

impl Mul for NumberValue {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int(a), Self::Int(b)) => Self::Int(a * b),
            (a, b) => Self::Float(a.to_f64() * b.to_f64()).normalize(),
        }
    }
}

impl Div for NumberValue {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Self::Float(self.to_f64() / rhs.to_f64()).normalize()
    }
}

impl From<i64> for NumberValue {
    fn from(v: i64) -> Self { Self::Int(v) }
}

impl From<f64> for NumberValue {
    fn from(v: f64) -> Self { Self::Float(v) }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataType {
    Bool,
    Number,
    String,
    Time,
    DateTime,
    Duration,
    Color,
    List(Box<DataType>),
    Object,
    Any,
    /// Plugin-declared type by full name (`ha.state`); definition lives in `TypeRegistry`.
    Named(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectField {
    pub name: String,
    pub description: String,
    pub value: DataValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataValue {
    Bool(bool),
    Number(NumberValue),
    String(String),
    Time(NaiveTime),
    DateTime(DateTime<Utc>),
    Duration(Duration),
    Color(Color),
    List {
        element_type: DataType,
        elements: Vec<DataValue>,
    },
    Object {
        fields: BTreeMap<String, ObjectField>,
        /// Name of a custom struct type; `None` for an untyped object.
        /// Without `skip_serializing_if`: postcard is not self-describing.
        #[serde(default)]
        type_name: Option<String>,
    },
}

impl DataValue {
    pub fn get_type(&self) -> DataType {
        match self {
            Self::Bool(_) => DataType::Bool,
            Self::Number(_) => DataType::Number,
            Self::String(_) => DataType::String,
            Self::Time(_) => DataType::Time,
            Self::DateTime(_) => DataType::DateTime,
            Self::Duration(_) => DataType::Duration,
            Self::Color(_) => DataType::Color,
            Self::List { element_type, .. } => DataType::List(Box::new(element_type.clone())),
            Self::Object { type_name: Some(name), .. } => DataType::Named(name.clone()),
            Self::Object { .. } => DataType::Object,
        }
    }
}

impl From<bool> for DataValue { fn from(v: bool) -> Self { Self::Bool(v) } }
impl From<f64> for DataValue { fn from(v: f64) -> Self { Self::Number(NumberValue::Float(v).normalize()) } }
impl From<i64> for DataValue { fn from(v: i64) -> Self { Self::Number(NumberValue::Int(v)) } }
impl From<&str> for DataValue { fn from(v: &str) -> Self { Self::String(v.into()) } }
impl From<String> for DataValue { fn from(v: String) -> Self { Self::String(v) } }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PinDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PinType {
    Exec,
    Data(DataType),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PinDefinition {
    pub id: PinId,
    pub name: String,
    pub pin_type: PinType,
    pub direction: PinDirection,
    /// Required input: without a connected source or an inline value on the node, the node does not execute.
    #[serde(default)]
    pub required: bool,
}

impl PinDefinition {
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeDefinition {
    pub type_name: String,
    pub title: String,
    pub category: String,
    pub description: String,
    /// Detailed explanation of how the node works ("info" tooltip in the editor).
    #[serde(default)]
    pub help: String,
    pub pins: Vec<PinDefinition>,
}

/// An unconnected input data pin's value is stored among the node's properties
/// under the name `in:<pin id>`; the engine substitutes it while nothing is connected to the pin.
const INPUT_VALUE_PREFIX: &str = "in:";

pub fn input_value_property(pin: PinId) -> String {
    format!("{INPUT_VALUE_PREFIX}{}", pin.0)
}

pub fn parse_input_value_property(name: &str) -> Option<PinId> {
    name.strip_prefix(INPUT_VALUE_PREFIX)?.parse().ok().map(PinId)
}

/// Whether an input of this type can be given a value directly on the node (entered as text, see `text`;
/// for an enum, selected from a list).
pub fn input_value_editable(types: &TypeRegistry, t: &DataType) -> bool {
    text::is_text_type_in(types, t)
}

/// Whether `value` is valid as an inline input value of type `t` on a node.
pub fn input_value_fits(types: &TypeRegistry, t: &DataType, value: &DataValue) -> bool {
    if let Some(def) = types.definition_of(t) {
        return matches!(value, DataValue::String(s) if def.values().iter().any(|v| &v.value == s));
    }
    let value_type = value.get_type();
    input_value_editable(types, t) && text::is_text_type(&value_type) && types.compatible(&value_type, t)
}

/// Whether an output of type `src` can be connected to an input of type `dst` when there are no custom types:
/// equal, or at least one is `Any` (also inside lists: `list<number>` → `list<any>`).
/// With custom types, use `TypeRegistry::compatible`.
pub fn types_compatible(src: &DataType, dst: &DataType) -> bool {
    TypeRegistry::default().compatible(src, dst)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MathOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LogOp { And, Or, Not }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ConnectionModel {
    pub src_node: NodeId,
    pub src_pin: PinId,
    pub dst_node: NodeId,
    pub dst_pin: PinId,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// Persisted state of a single node: type + properties from which it can be recreated via the registry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeModel {
    pub id: NodeId,
    pub type_name: String,
    pub properties: Vec<(String, DataValue)>,
    pub position: Position,
}

/// Complete graph (or a fragment for copying) in a format independent of node implementations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GraphModel {
    pub nodes: Vec<NodeModel>,
    pub exec_connections: Vec<ConnectionModel>,
    pub data_connections: Vec<ConnectionModel>,
}

/// A node paired with its current definition (pins may depend on properties).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInstance {
    pub model: NodeModel,
    pub definition: NodeDefinition,
    /// Stub node: the plugin for its type is unavailable (disabled or not loaded). The message is an explanation
    /// for the editor. Properties and pins are preserved, but the node cannot execute or be edited.
    #[serde(default)]
    pub unavailable: Option<UiMessage>,
}

// --- Server <-> editor protocol (postcard over WebSocket) ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ClientMessage {
    AddNode { type_name: String, position: Position },
    RemoveNode(NodeId),
    MoveNode { id: NodeId, position: Position },
    SetProperty { id: NodeId, name: String, value: DataValue },
    ConnectExec(ConnectionModel),
    DisconnectExec(ConnectionModel),
    ConnectData(ConnectionModel),
    DisconnectData { dst_node: NodeId, dst_pin: PinId },
    TriggerNode { id: NodeId, pin: PinId },
    /// Value of an unconnected data input; `None` clears it (the node falls back to its default).
    SetInputValue { id: NodeId, pin: PinId, value: Option<DataValue> },
    /// Paste a graph fragment (copy/duplicate). Nodes receive new IDs;
    /// positions are taken from the fragment as-is.
    Paste(GraphModel),
    /// Enable or disable a plugin on the fly (without restarting the server).
    SetPluginEnabled { name: String, enabled: bool },
    /// Save plugin settings. Keys not present in the list remain unchanged.
    ConfigurePlugin { name: String, values: Vec<(String, SettingUpdate)> },
}

/// What a node received and emitted during its last execution (for the editor inspector).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeTrace {
    pub node_id: NodeId,
    pub tick: u64,
    /// Connected data inputs; `None` means the source had no value yet.
    pub inputs: Vec<(PinId, Option<DataValue>)>,
    /// Known values of output data pins after execution.
    pub outputs: Vec<(PinId, DataValue)>,
    /// Exec outputs that fired.
    pub fired: Vec<PinId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// Full state snapshot — sent immediately after connecting.
    Init {
        definitions: Vec<NodeDefinition>,
        nodes: Vec<NodeInstance>,
        exec_connections: Vec<ConnectionModel>,
        data_connections: Vec<ConnectionModel>,
        /// Latest node executions and how many milliseconds ago they occurred.
        traces: Vec<(NodeTrace, u64)>,
        /// Custom data types declared by plugins.
        types: Vec<TypeDefinition>,
        /// All discovered plugins with their state and settings (secrets only indicate whether set).
        plugins: Vec<PluginInfo>,
        /// Translation resources of the engine core and every enabled plugin, merged per locale.
        translations: Translations,
    },
    NodeAdded(NodeInstance),
    NodeRemoved(NodeId),
    NodeMoved { id: NodeId, position: Position },
    /// Properties changed (the definition may have changed as well).
    NodeUpdated(NodeInstance),
    ExecConnected(ConnectionModel),
    ExecDisconnected(ConnectionModel),
    DataConnected(ConnectionModel),
    DataDisconnected { dst_node: NodeId, dst_pin: PinId },
    /// Nodes executed on the latest tick, with their inputs and outputs.
    Executed(Vec<NodeTrace>),
    /// Sent only to the initiator of `AddNode` / `Paste`: IDs of created nodes (to select them).
    /// Arrives after the corresponding `NodeAdded` messages.
    Created(Vec<NodeId>),
    Log { node_id: NodeId, message: String },
    /// Plugin state or settings changed.
    PluginUpdated(PluginInfo),
    /// The set of node and data types changed (plugin enabled or disabled): new definitions for the palette.
    /// Affected nodes arrive separately as `NodeUpdated`.
    Catalog { definitions: Vec<NodeDefinition>, types: Vec<TypeDefinition>, translations: Translations },
    /// Error for the client that caused it; translatable.
    Error(UiMessage),
}

pub fn parse_data_type(s: &str) -> Option<DataType> {
    Some(match s.trim().to_lowercase().as_str() {
        "bool" => DataType::Bool,
        "number" => DataType::Number,
        "string" => DataType::String,
        "time" => DataType::Time,
        "datetime" => DataType::DateTime,
        "duration" => DataType::Duration,
        "color" => DataType::Color,
        "object" => DataType::Object,
        "any" => DataType::Any,
        other => {
            if let Some(inner) = other.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
                return parse_data_type(inner).map(|t| DataType::List(Box::new(t)));
            }
            // Custom type: case is preserved (type names are lowercase, but we do not guess)
            let name = s.trim();
            return is_type_name(name).then(|| DataType::Named(name.to_string()));
        }
    })
}

pub fn data_type_name(t: &DataType) -> String {
    match t {
        DataType::Bool => "bool".into(),
        DataType::Number => "number".into(),
        DataType::String => "string".into(),
        DataType::Time => "time".into(),
        DataType::DateTime => "datetime".into(),
        DataType::Duration => "duration".into(),
        DataType::Color => "color".into(),
        DataType::Object => "object".into(),
        DataType::Any => "any".into(),
        DataType::Named(name) => name.clone(),
        DataType::List(inner) => format!("list<{}>", data_type_name(inner)),
    }
}


/// Permissions requested by a plugin in its manifest. Enforced by the host on every plugin call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    /// HTTP(S) to listed domains and their subdomains; `*` — any domain.
    Http { allowed_domains: Vec<String> },
    /// Plugin's own isolated directory (via the WASI filesystem).
    Storage,
    /// Persistent `ws://` / `wss://` connections to listed hosts and their subdomains; `*` — any host.
    WebSocket { allowed_hosts: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub requested_capabilities: Vec<Capability>,
    /// Schema of settings configured by the user in the UI (URL, token, etc.).
    pub settings: Vec<SettingDefinition>,
}

impl PluginManifest {
    pub fn new(name: &str, version: &str, author: &str) -> Self {
        Self { name: name.into(), version: version.into(), author: author.into(), requested_capabilities: Vec::new(), settings: Vec::new() }
    }

    pub fn capability(mut self, capability: Capability) -> Self {
        self.requested_capabilities.push(capability);
        self
    }

    pub fn setting(mut self, setting: SettingDefinition) -> Self {
        self.settings.push(setting);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeOutput {
    Execution(PinId),
    Data(PinId, DataValue),
    Variable(String, DataValue),
}


