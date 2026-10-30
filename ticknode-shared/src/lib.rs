use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{NaiveTime, DateTime, Utc};
use std::time::Duration;
use std::ops::{Add, Sub, Mul, Div};
use std::collections::BTreeMap;

pub mod condition;
pub mod text;
pub mod plugin;
pub mod types;

pub use plugin::*;
pub use types::*;

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
    /// Тип, оголошений плагіном, за повною назвою (`ha.state`); визначення — у `TypeRegistry`.
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
        /// Назва власного типу-структури; `None` — довільний об'єкт.
        /// Без `skip_serializing_if`: postcard не самоописовий.
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
    /// Обов'язковий вхід: без під'єднаного джерела чи значення на ноді нода не виконується.
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
    /// Детальне пояснення роботи ноди (підказка «інфо» в редакторі).
    #[serde(default)]
    pub help: String,
    pub pins: Vec<PinDefinition>,
}

/// Значення непід'єднаного вхідного піна даних зберігається серед властивостей ноди
/// під іменем `in:<id піна>`; рушій підставляє його, поки до піна нічого не під'єднано.
const INPUT_VALUE_PREFIX: &str = "in:";

pub fn input_value_property(pin: PinId) -> String {
    format!("{INPUT_VALUE_PREFIX}{}", pin.0)
}

pub fn parse_input_value_property(name: &str) -> Option<PinId> {
    name.strip_prefix(INPUT_VALUE_PREFIX)?.parse().ok().map(PinId)
}

/// Чи можна задати значення входу такого типу прямо на ноді (набирається текстом, див. `text`;
/// для переліку — вибирається зі списку).
pub fn input_value_editable(types: &TypeRegistry, t: &DataType) -> bool {
    text::is_text_type_in(types, t)
}

/// Чи підходить значення як значення входу типу `t` на ноді.
pub fn input_value_fits(types: &TypeRegistry, t: &DataType, value: &DataValue) -> bool {
    if let Some(def) = types.definition_of(t) {
        return matches!(value, DataValue::String(s) if def.values().iter().any(|v| &v.value == s));
    }
    let value_type = value.get_type();
    input_value_editable(types, t) && text::is_text_type(&value_type) && types.compatible(&value_type, t)
}

/// Чи можна з'єднати вихід типу `src` зі входом типу `dst`, коли власних типів немає:
/// однакові, або хоч один — Any (також усередині списків: `list<number>` → `list<any>`).
/// З власними типами — `TypeRegistry::compatible`.
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

/// Збережений стан однієї ноди: тип + властивості, з яких її можна відтворити через реєстр.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeModel {
    pub id: NodeId,
    pub type_name: String,
    pub properties: Vec<(String, DataValue)>,
    pub position: Position,
}

/// Повний граф (або фрагмент для копіювання) у форматі, незалежному від реалізації нод.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GraphModel {
    pub nodes: Vec<NodeModel>,
    pub exec_connections: Vec<ConnectionModel>,
    pub data_connections: Vec<ConnectionModel>,
}

/// Нода разом з її поточною дефініцією (піни можуть залежати від властивостей).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInstance {
    pub model: NodeModel,
    pub definition: NodeDefinition,
    /// Нода-заглушка: плагін її типу недоступний (вимкнений, не завантажений). Текст — пояснення
    /// для редактора. Властивості й піни збережені, але нода не виконується й не редагується.
    #[serde(default)]
    pub unavailable: Option<String>,
}

// --- Протокол сервер <-> редактор (postcard поверх WebSocket) ---

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
    /// Значення непід'єднаного входу даних; `None` — прибрати (нода візьме своє за замовчуванням).
    SetInputValue { id: NodeId, pin: PinId, value: Option<DataValue> },
    /// Вставити фрагмент графа (копіювання/дублювання). Ноди отримують нові ID,
    /// позиції беруться з фрагмента як є.
    Paste(GraphModel),
    /// Увімкнути чи вимкнути плагін на льоту (без перезапуску сервера).
    SetPluginEnabled { name: String, enabled: bool },
    /// Зберегти налаштування плагіна. Ключі, яких немає в списку, лишаються як були.
    ConfigurePlugin { name: String, values: Vec<(String, SettingUpdate)> },
}

/// Що нода отримала й видала під час останнього виконання (для інспектора редактора).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeTrace {
    pub node_id: NodeId,
    pub tick: u64,
    /// Підключені входи даних; `None` — джерело ще не мало значення.
    pub inputs: Vec<(PinId, Option<DataValue>)>,
    /// Відомі значення вихідних пінів даних після виконання.
    pub outputs: Vec<(PinId, DataValue)>,
    /// Exec-виходи, що спрацювали.
    pub fired: Vec<PinId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServerMessage {
    /// Повний знімок стану — надсилається одразу після підключення.
    Init {
        definitions: Vec<NodeDefinition>,
        nodes: Vec<NodeInstance>,
        exec_connections: Vec<ConnectionModel>,
        data_connections: Vec<ConnectionModel>,
        /// Останні виконання нод і скільки мілісекунд тому вони були.
        traces: Vec<(NodeTrace, u64)>,
        /// Власні типи даних, оголошені плагінами.
        types: Vec<TypeDefinition>,
        /// Усі знайдені плагіни зі станом і налаштуваннями (секрети — лише «задано/не задано»).
        plugins: Vec<PluginInfo>,
    },
    NodeAdded(NodeInstance),
    NodeRemoved(NodeId),
    NodeMoved { id: NodeId, position: Position },
    /// Властивості змінились (дефініція могла змінитись теж).
    NodeUpdated(NodeInstance),
    ExecConnected(ConnectionModel),
    ExecDisconnected(ConnectionModel),
    DataConnected(ConnectionModel),
    DataDisconnected { dst_node: NodeId, dst_pin: PinId },
    /// Ноди, що виконались за останній такт, з їхніми входами й виходами.
    Executed(Vec<NodeTrace>),
    /// Лише ініціатору `AddNode` / `Paste`: ID створених нод (щоб їх виділити).
    /// Приходить після відповідних `NodeAdded`.
    Created(Vec<NodeId>),
    Log { node_id: NodeId, message: String },
    /// Стан чи налаштування плагіна змінились.
    PluginUpdated(PluginInfo),
    /// Набір типів нод і даних змінився (плагін увімкнено чи вимкнено): нові дефініції для палітри.
    /// Змінені ноди приходять окремо як `NodeUpdated`.
    Catalog { definitions: Vec<NodeDefinition>, types: Vec<TypeDefinition> },
    Error(String),
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
            // Власний тип: регістр зберігається (назви типів — малі літери, але не вгадуємо)
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


/// Права, які плагін запитує в маніфесті. Хост перевіряє їх при кожному зверненні плагіна.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    /// HTTP(S) до доменів зі списку та їхніх піддоменів; `*` — будь-який домен.
    Http { allowed_domains: Vec<String> },
    /// Власний ізольований каталог плагіна (через WASI-файлову систему).
    Storage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub requested_capabilities: Vec<Capability>,
    /// Схема налаштувань, які користувач задає в інтерфейсі (URL, токен…).
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


