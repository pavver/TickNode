//! Node geometry in world coordinates. Values must match `styles/nodes.css`
//! (`.node-header`, `.node-props`, `.pins`, `.pin`) because wires are routed using computed
//! rather than DOM-measured pin positions.

use ticknode_shared::condition::{MatchCase, MatchInput};
use ticknode_shared::{
    DataType, DataValue, NodeDefinition, PinDefinition, PinDirection, PinId, PinType, Position,
    TypeRegistry, parse_input_value_property,
};

mod routes;
mod layout;
pub use layout::{NodeLayout, resize_size};
pub use routes::{route_path, route_insert_index, route_midpoint};

/// Connection presentation: solid for compatible pins, dashed for conversion, absent if invalid.
pub fn wire_conversion(types: &TypeRegistry, src: &PinType, dst: &PinType) -> Option<bool> {
    match (src, dst) {
        (PinType::Exec, PinType::Exec) => Some(false),
        (PinType::Data(a), PinType::Data(b)) if types.compatible(a, b) => Some(false),
        (PinType::Data(a), PinType::Data(b)) if types.convertible(a, b).is_some() => Some(true),
        _ => None,
    }
}

pub const DU: f64 = 4.0;
pub const NODE_WIDTH: f64 = 60.0 * DU;
/// Match is wider: each condition row holds an input, an operator, and an operand.
pub const MATCH_WIDTH: f64 = 100.0 * DU;
const HEADER_H: f64 = 8.0 * DU;
const BODY_PAD: f64 = 1.0 * DU;
pub const ROW_H: f64 = 6.0 * DU;
/// Height of a node with a single pin row (or without a definition).
pub const MIN_NODE_HEIGHT: f64 = HEADER_H + 2.0 * BODY_PAD + ROW_H;

pub const MIN_ZOOM: f64 = 0.25;
pub const MAX_ZOOM: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0 }
    }
}

impl Camera {
    pub fn to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        ((sx - self.x) / self.zoom, (sy - self.y) / self.zoom)
    }

    /// Zooms while keeping the point under the cursor fixed.
    pub fn zoom_at(&self, sx: f64, sy: f64, factor: f64) -> Camera {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let (wx, wy) = self.to_world(sx, sy);
        Camera { x: sx - wx * zoom, y: sy - wy * zoom, zoom }
    }
}

pub fn pins(def: &NodeDefinition, direction: PinDirection) -> impl Iterator<Item = &PinDefinition> {
    def.pins.iter().filter(move |p| p.direction == direction)
}

/// Property shown and editable directly on the node (a row between the header and pins).
/// Input values (`in:*`) are rendered next to pins, and Match cases are rendered in their own block.
pub fn is_inline_property(name: &str, value: &DataValue) -> bool {
    ticknode_shared::text::is_text_type(&value.get_type()) && parse_input_value_property(name).is_none()
        && ticknode_shared::parse_split_pin_property(name).is_none()
}

/// Number of inline property rows on a node.
pub fn prop_rows(properties: &[(String, DataValue)]) -> usize {
    properties.iter().filter(|(n, v)| is_inline_property(n, v)).count()
}

/// Match node cases extracted from its properties (invalid entries are skipped, just as on the server).
pub fn match_cases(properties: &[(String, DataValue)]) -> Vec<MatchCase> {
    match properties.iter().find(|(n, _)| n == "cases").map(|(_, v)| v) {
        Some(DataValue::List { elements, .. }) => elements.iter().filter_map(|e| match e {
            DataValue::String(s) => MatchCase::parse(s).ok(),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    }
}

/// Explicit inputs of a Match node (`inputs` property).
pub fn match_inputs_of(properties: &[(String, DataValue)]) -> Vec<MatchInput> {
    match properties.iter().find(|(n, _)| n == "inputs").map(|(_, v)| v) {
        Some(DataValue::List { elements, .. }) => elements.iter().filter_map(|e| match e {
            DataValue::String(s) => MatchInput::parse(s).ok(),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    }
}

pub fn node_width(type_name: &str) -> f64 {
    if type_name == "match" { MATCH_WIDTH } else { NODE_WIDTH }
}

/// Row count in a Match case block: header (with exec output) + conditions (or an "always" row when empty).
pub fn case_rows(case: &MatchCase) -> usize {
    1 + case.conditions.len().max(1)
}

pub fn pin_type(def: &NodeDefinition, pin: PinId, direction: PinDirection) -> Option<&PinType> {
    pins(def, direction).find(|p| p.id == pin).map(|p| &p.pin_type)
}

/// Horizontal cubic Bézier curve from output to input.
pub fn wire_path((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> String {
    route_path((x1, y1), &[], (x2, y2))
}

/// CSS color of a pin by its type.
pub fn pin_color(types: &TypeRegistry, pin_type: &PinType) -> String {
    match pin_type {
        PinType::Exec => "var(--text-primary)".to_string(),
        PinType::Data(t) => type_color(types, t),
    }
}

/// Color of a data type: custom types use `color` from their definition, falling back to a
/// deterministic color hashed from the type name.
pub fn type_color(types: &TypeRegistry, t: &DataType) -> String {
    match t {
        DataType::Named(name) => types.get(name)
            .and_then(|d| d.color.as_deref())
            .filter(|c| is_hex_color(c))
            .map_or_else(|| hash_color(name), str::to_string),
        other => data_type_color(other).to_string(),
    }
}

/// Only `#rrggbb` colors are allowed into CSS (definitions come from plugins).
fn is_hex_color(c: &str) -> bool {
    c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit())
}

/// Deterministic color from a name hash (FNV-1a → hue).
fn hash_color(name: &str) -> String {
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    format!("hsl({} 55% 55%)", (hash >> 16) % 360)
}

pub fn data_type_color(t: &DataType) -> &'static str {
    match t {
        DataType::Bool => "var(--error)",
        DataType::Number => "var(--accent)",
        DataType::String => "var(--warning)",
        DataType::Time | DataType::DateTime | DataType::Duration => "var(--info)",
        DataType::Color => "var(--magenta)",
        DataType::List(_) => "var(--success)",
        DataType::Object => "var(--text-secondary)",
        DataType::Any => "var(--border-strong)",
        DataType::Named(_) => "var(--text-secondary)",
    }
}

#[cfg(test)]
mod tests;
