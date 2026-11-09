use serde::{Deserialize, Serialize};
use crate::{DataType, DataValue, PropertyDefinition, TypeDefinition, TypeRegistry, text};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinId(pub u32);

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
    /// Schema of properties shown on the node as a drop-down or picker. Properties that are not listed
    /// keep the default text row.
    #[serde(default)]
    pub properties: Vec<PropertyDefinition>,
    /// Relative work per call; native logic costs one unit.
    #[serde(default = "default_cost")]
    pub cost: u32,
    /// Can adapt its pin type when created from an existing data pin.
    #[serde(default)]
    pub adaptive: bool,
}

fn default_cost() -> u32 { 1 }

/// Lets plugin code fill only the fields it cares about (`..Default::default()`), so adding a
/// field to definitions does not break existing plugins at the source level. The cost defaults to 1.
impl Default for NodeDefinition {
    fn default() -> Self {
        Self {
            type_name: String::new(), title: String::new(), category: String::new(), description: String::new(),
            help: String::new(), pins: Vec::new(), properties: Vec::new(), cost: default_cost(), adaptive: false,
        }
    }
}

impl NodeDefinition {
    /// Set relative work for this definition (also re-exported by the plugin SDK).
    pub fn cost(mut self, cost: u32) -> Self {
        self.cost = cost;
        self
    }
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

/// The 0x8000_0000..=0xbfff_ffff range belongs to engine-generated struct field pins.
pub const VIRTUAL_PIN_BIT: u32 = 0x8000_0000;
pub const VIRTUAL_PIN_MASK: u32 = 0xc000_0000;

pub fn is_virtual_pin(pin: PinId) -> bool {
    pin.0 & VIRTUAL_PIN_MASK == VIRTUAL_PIN_BIT
}

/// One level of splitting: 22 bits for the original pin, 8 bits for the field index.
pub fn split_field_pin(pin: PinId, index: usize) -> Option<PinId> {
    (pin.0 <= 0x3f_ffff && index < 256)
        .then_some(PinId(VIRTUAL_PIN_BIT | (pin.0 << 8) | index as u32))
}

pub fn split_field_parent(pin: PinId) -> Option<(PinId, usize)> {
    is_virtual_pin(pin)
        .then_some((PinId((pin.0 & !VIRTUAL_PIN_MASK) >> 8), (pin.0 & 255) as usize))
}

pub fn split_pin_property(pin: PinId) -> String {
    format!("split:{}", pin.0)
}

pub fn parse_split_pin_property(name: &str) -> Option<PinId> {
    name.strip_prefix("split:")?.parse().ok().map(PinId)
}

/// Only nonempty structs representable in the reserved id range can be split.
pub fn splittable_type<'a>(types: &'a TypeRegistry, pin: &PinDefinition) -> Option<&'a TypeDefinition> {
    let PinType::Data(t) = &pin.pin_type else { return None };
    let def = types.definition_of(t).filter(|d| d.is_struct())?;
    (!def.fields().is_empty() && split_field_pin(pin.id, def.fields().len() - 1).is_some()).then_some(def)
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


#[cfg(test)]
mod tests;
