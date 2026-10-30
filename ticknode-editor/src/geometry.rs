//! Node geometry in world coordinates. Values must match `style.css`
//! (`.node-header`, `.node-props`, `.pins`, `.pin`) because wires are routed using computed
//! rather than DOM-measured pin positions.

use ticknode_shared::condition::{MatchCase, MatchInput};
use ticknode_shared::{
    DataType, DataValue, NodeDefinition, NodeInstance, PinDefinition, PinDirection, PinId, PinType, Position,
    TypeRegistry, parse_input_value_property,
};

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

/// Node layout: pin y-offsets (row center from the node top) and total height.
/// Regular node: header, property rows, pin rows (inputs on the left, outputs on the right).
/// Match: In, input rows, "+ input" row, then case blocks — each output sits in its block header,
/// and Else is on the last row.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    pub width: f64,
    pub height: f64,
    pub pins_top: f64,
    /// Top offset of the Match cases block.
    pub cases_top: f64,
    inputs: Vec<(PinId, f64)>,
    outputs: Vec<(PinId, f64)>,
}

impl NodeLayout {
    pub fn new(def: &NodeDefinition, properties: &[(String, DataValue)]) -> Self {
        let pins_top = HEADER_H + BODY_PAD + prop_rows(properties) as f64 * ROW_H;
        let row_y = |i: usize| pins_top + i as f64 * ROW_H + ROW_H / 2.0;
        let inputs: Vec<_> = pins(def, PinDirection::Input).enumerate().map(|(i, p)| (p.id, row_y(i))).collect();
        if def.type_name == "match" {
            let cases_top = pins_top + (inputs.len() + 1) as f64 * ROW_H;
            let mut y = cases_top;
            let mut outputs = Vec::new();
            for case in match_cases(properties) {
                outputs.push((PinId(case.pin), y + ROW_H / 2.0));
                y += case_rows(&case) as f64 * ROW_H;
            }
            // "+ case" row with the Else output
            outputs.push((PinId(0), y + ROW_H / 2.0));
            y += ROW_H;
            return Self { width: MATCH_WIDTH, height: y + BODY_PAD, pins_top, cases_top, inputs, outputs };
        }
        let outputs: Vec<_> = pins(def, PinDirection::Output).enumerate().map(|(i, p)| (p.id, row_y(i))).collect();
        let rows = inputs.len().max(outputs.len()).max(1);
        let height = pins_top + rows as f64 * ROW_H + BODY_PAD;
        Self { width: node_width(&def.type_name), height, pins_top, cases_top: height, inputs, outputs }
    }

    pub fn of(instance: &NodeInstance) -> Self {
        Self::new(&instance.definition, &instance.model.properties)
    }

    /// Wire attachment point for a pin (socket center on the node edge).
    pub fn anchor(&self, pos: Position, pin: PinId, direction: PinDirection) -> Option<(f64, f64)> {
        let (list, x) = match direction {
            PinDirection::Input => (&self.inputs, pos.x as f64),
            PinDirection::Output => (&self.outputs, pos.x as f64 + self.width),
        };
        list.iter().find(|(p, _)| *p == pin).map(|(_, y)| (x, pos.y as f64 + y))
    }
}

/// Width and height of a node instance.
pub fn instance_size(instance: &NodeInstance) -> (f64, f64) {
    let l = NodeLayout::of(instance);
    (l.width, l.height)
}

pub fn pin_type(def: &NodeDefinition, pin: PinId, direction: PinDirection) -> Option<&PinType> {
    pins(def, direction).find(|p| p.id == pin).map(|p| &p.pin_type)
}

/// Horizontal cubic Bézier curve from output to input.
pub fn wire_path((x1, y1): (f64, f64), (x2, y2): (f64, f64)) -> String {
    let dx = ((x2 - x1).abs() / 2.0).max(10.0 * DU);
    format!("M {x1} {y1} C {} {y1}, {} {y2}, {x2} {y2}", x1 + dx, x2 - dx)
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
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_cursor_point_fixed() {
        let view = Camera { x: 30.0, y: -20.0, zoom: 1.0 };
        let before = view.to_world(200.0, 150.0);
        let after = view.zoom_at(200.0, 150.0, 1.5).to_world(200.0, 150.0);
        assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
    }

    fn def(type_name: &str, pins: Vec<(u32, PinType, PinDirection)>) -> NodeDefinition {
        NodeDefinition {
            type_name: type_name.into(), title: "T".into(), category: "Data".into(), description: String::new(), help: String::new(),
            pins: pins.into_iter().map(|(id, pin_type, direction)| PinDefinition { id: PinId(id), name: "p".into(), pin_type, direction, required: false }).collect(),
        }
    }

    #[test]
    fn property_rows_shift_pins() {
        use ticknode_shared::NumberValue;
        let d = def("t", vec![(0, PinType::Data(DataType::Number), PinDirection::Output)]);
        let props = vec![
            ("value".to_string(), DataValue::Number(NumberValue::Int(1))),
            ("element_type".to_string(), DataValue::String("number".into())),
            ("in:1".to_string(), DataValue::Bool(true)),
        ];
        assert_eq!(prop_rows(&props), 2);
        let pos = Position { x: 0.0, y: 0.0 };
        let (_, y0) = NodeLayout::new(&d, &[]).anchor(pos, PinId(0), PinDirection::Output).unwrap();
        let (_, y2) = NodeLayout::new(&d, &props).anchor(pos, PinId(0), PinDirection::Output).unwrap();
        assert_eq!(y2 - y0, 2.0 * ROW_H);
        assert_eq!(NodeLayout::new(&d, &props).height - NodeLayout::new(&d, &[]).height, 2.0 * ROW_H);
        assert_eq!(NodeLayout::new(&d, &[]).height, MIN_NODE_HEIGHT);
    }

    #[test]
    fn match_outputs_sit_on_case_headers() {
        let d = def("match", vec![
            (0, PinType::Exec, PinDirection::Input),
            (1, PinType::Data(DataType::Any), PinDirection::Input),
            (3, PinType::Exec, PinDirection::Output),
            (7, PinType::Exec, PinDirection::Output),
            (0, PinType::Exec, PinDirection::Output),
        ]);
        let cases = DataValue::List { element_type: DataType::String, elements: vec![
            DataValue::String("3: a > 1 and a < 5".into()),
            DataValue::String("7:".into()),
        ] };
        let layout = NodeLayout::new(&d, &[("cases".into(), cases)]);
        let y = |pin| layout.anchor(Position::default(), PinId(pin), PinDirection::Output).unwrap().1;
        let top = layout.cases_top;
        // In, input, "+ input"
        assert_eq!(top, layout.pins_top + 3.0 * ROW_H);
        assert_eq!(y(3), top + ROW_H / 2.0);
        // Case 3: header + 2 conditions; case 7: header + "always"
        assert_eq!(y(7), top + 3.0 * ROW_H + ROW_H / 2.0);
        assert_eq!(y(0), top + 5.0 * ROW_H + ROW_H / 2.0);
        assert_eq!(layout.height, top + 6.0 * ROW_H + BODY_PAD);
    }

    #[test]
    fn named_type_colors() {
        use ticknode_shared::TypeDefinition;
        let mut types = TypeRegistry::new();
        types.insert_all(&[
            TypeDefinition::new_enum("p.colored", "C", &[("a", "A")]).colored("#e0a030"),
            TypeDefinition::new_enum("p.plain", "P", &[("a", "A")]),
            TypeDefinition::new_enum("p.evil", "E", &[("a", "A")]).colored("red;} body{display:none"),
        ]).unwrap();
        let color = |n: &str| type_color(&types, &DataType::Named(n.into()));
        assert_eq!(color("p.colored"), "#e0a030");
        assert!(color("p.plain").starts_with("hsl("));
        assert_eq!(color("p.plain"), color("p.plain"), "колір стабільний");
        assert!(color("p.evil").starts_with("hsl("), "некоректний колір не потрапляє в CSS");
        assert_ne!(color("p.plain"), color("p.evil"));
        assert_eq!(type_color(&types, &DataType::Number), "var(--accent)");
    }

    #[test]
    fn zoom_is_clamped() {
        assert_eq!(Camera::default().zoom_at(0.0, 0.0, 100.0).zoom, MAX_ZOOM);
        assert_eq!(Camera::default().zoom_at(0.0, 0.0, 0.001).zoom, MIN_ZOOM);
    }
}
