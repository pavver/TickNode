use crate::{NodeDefinition, DataValue, Trigger, Inputs, Output, PinId, PluginManifest, NodeId, TypeDefinition, UiError, WsEvent, Emission};

/// A plugin node. One instance corresponds to one node in the graph.
pub trait PluginNode: 'static {
    /// Pins may depend on properties.
    fn definition(&self) -> NodeDefinition;
    /// Properties used to reconstruct the node (graph persistence, plugin restart).
    fn properties(&self) -> Vec<(String, DataValue)> { Vec::new() }
    /// Validates interactive edits; stored state restoration bypasses this hook.
    fn validate_property(&self, _name: &str, _value: &DataValue) -> Result<(), UiError> { Ok(()) }
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
