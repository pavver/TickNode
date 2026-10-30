//! SDK для плагінів нод TickNode.
//!
//! Плагін — WASM-компонент (`crate-type = ["cdylib"]`, ціль `wasm32-wasip2`) з контрактом
//! `wit/plugin.wit`. Реалізуйте [`Plugin`] і [`PluginNode`], потім викличте [`export_plugin!`].
//! Кожна нода графа — окремий екземпляр [`PluginNode`], тож стан можна тримати в полях.
//!
//! ```ignore
//! use ticknode_plugin_sdk::*;
//!
//! struct MyPlugin;
//! impl Plugin for MyPlugin {
//!     const NODE_TYPES: &'static [&'static str] = &["my/node"];
//!     fn manifest() -> PluginManifest { /* ... */ }
//!     fn create(type_name: &str) -> Option<Box<dyn PluginNode>> { /* ... */ }
//! }
//! export_plugin!(MyPlugin);
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use std::collections::BTreeMap;

pub use ticknode_shared::{
    Capability, DataType, DataValue, EnumValue, FieldDefinition, NodeDefinition, NumberValue, ObjectField,
    PinDefinition, PinDirection, PinId, PinType, PluginManifest, SettingDefinition, SettingKind, TypeDefinition, TypeKind,
};

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
    /// 0 — типовий тайм-аут хоста.
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

/// Причина виконання ноди.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trigger {
    /// Імпульс на вхідний exec-пін (або ручний запуск з редактора).
    Pin(PinId),
    /// Завершилась задача, запущена цією нодою.
    Task { id: TaskId, result: Result<TaskOutput, String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    /// Імпульс з вихідного exec-піна.
    Exec(PinId),
    /// Значення вихідного піна даних.
    Data(PinId, DataValue),
}

/// Значення підключених вхідних пінів даних.
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

    /// Поле значення-структури на вході `pin`.
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

    /// Рядок або порожній рядок, якщо пін не підключено чи тип інший.
    pub fn string(&self, pin: PinId) -> String {
        match self.get(pin) {
            Some(DataValue::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

/// Побудова значення власного типу-структури: `StructValue::new("ha.state").field("state", "on").build()`.
/// Поля, яких немає в `TypeDefinition`, хост не перевіряє, тож імена мусять збігатися з оголошеними.
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

    /// Додає необов'язкове поле лише якщо значення є.
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

/// Поле значення-структури (або довільного об'єкта).
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

/// Нода плагіна. Один екземпляр — одна нода графа.
pub trait PluginNode: 'static {
    /// Піни можуть залежати від властивостей.
    fn definition(&self) -> NodeDefinition;
    /// Властивості, з яких ноду відтворюють (збереження графа, перезапуск плагіна).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    fn set_property(&mut self, _name: &str, _value: DataValue) {}
    fn execute(&mut self, trigger: Trigger, inputs: &Inputs) -> Vec<Output>;
    /// Для нод-джерел подій: викликається на початку кожного такту, якщо `polls()` — `true`.
    fn poll(&mut self, _tick: u64) -> Option<PinId> { None }
    /// Нода — джерело подій, і хост має викликати `poll` щотакту.
    fn polls(&self) -> bool { false }
}

pub trait Plugin: 'static {
    /// Усі типи нод плагіна; дефініції для палітри беруться з `create(type).definition()`.
    const NODE_TYPES: &'static [&'static str];
    fn manifest() -> PluginManifest;
    fn create(type_name: &str) -> Option<Box<dyn PluginNode>>;
    /// Власні типи даних плагіна (структури й переліки); піни нод можуть посилатись на них
    /// через `DataType::Named`. Для кожної структури хост сам додає ноди Make/Break.
    fn types() -> Vec<TypeDefinition> { Vec::new() }
    /// Застосовує налаштування за схемою `manifest().settings` (значення — збережені або типові).
    /// Хост викликає після завантаження, при кожній зміні й після перестворення екземпляра
    /// (аварія). Екземпляр на плагін один, тож налаштування тримають у статичному стані
    /// (`thread_local!`). `Err` показується користувачу, плагін лишається завантаженим.
    fn configure(_settings: &[(String, DataValue)]) -> Result<(), String> { Ok(()) }
}

/// Значення налаштування за ключем.
pub fn setting<'a>(settings: &'a [(String, DataValue)], key: &str) -> Option<&'a DataValue> {
    settings.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Текстове налаштування (порожній рядок вважається незаданим).
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

/// Сервіси хоста. Задачі (`delay_*`, `http`) можна запускати лише з `execute`/`poll`;
/// їхній результат прийде в `execute` як [`Trigger::Task`].
pub mod host {
    use super::*;

    pub fn log(message: &str) {
        wit_host::log(wit::LogLevel::Info, message);
    }

    pub fn warn(message: &str) {
        wit_host::log(wit::LogLevel::Warn, message);
    }

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

    /// Затримка, округлена вгору до цілих тактів.
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

    /// Скасовує задачу цієї ноди. `false`, якщо задача вже завершилась.
    pub fn cancel(task: TaskId) -> bool {
        wit_host::cancel_task(task)
    }
}

/// Експортує плагін як WASM-компонент. Викликається один раз у крейті плагіна.
#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        type __TickNodeExport = $crate::Exporter<$plugin>;
        $crate::bindings::__export_ticknode_plugin!(__TickNodeExport with_types_in $crate::bindings);
    };
}

// --- Реалізація WIT-експортів ---

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
            }).collect(),
            settings: m.settings.into_iter().map(to_wit_setting).collect(),
        }
    }

    fn configure(settings: Vec<wit::Property>) -> Result<(), String> {
        let settings: Vec<(String, DataValue)> = settings.into_iter().filter_map(|p| Some((p.name, decode(&p.value)?))).collect();
        P::configure(&settings)
    }

    fn definitions() -> Vec<wit::NodeDefinition> {
        P::NODE_TYPES.iter().filter_map(|t| P::create(t)).map(|n| to_wit_definition(n.definition(), n.polls())).collect()
    }

    fn types() -> Vec<wit::TypeDefinition> {
        P::types().into_iter().map(to_wit_type_definition).collect()
    }

    fn create_node(type_name: String) -> Result<wit_nodes::Node, String> {
        P::create(&type_name)
            .map(|node| wit_nodes::Node::new(NodeHandle(RefCell::new(node))))
            .ok_or_else(|| format!("Невідомий тип ноди: {type_name}"))
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
    postcard::to_allocvec(value).expect("DataValue завжди серіалізується")
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
        DataType::List(_) => unreachable!("списки розгорнуто вище"),
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
    }
}

/// Скорочення для опису пінів у `definition()`.
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
