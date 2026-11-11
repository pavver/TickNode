//! Shared layout for node rows, sockets, bounds and size previews.
use super::*;
use ticknode_shared::{NodeSize, PropertyKind};

/// Node layout: pin y-offsets (row center from the node top) and total height.
/// Regular node: header, property rows, pin rows (inputs on the left, outputs on the right).
/// Match: In, input rows, "+ input" row, then case blocks — each output sits in its block header,
/// and Else is on the last row.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    pub width: f64,
    pub(super) pin_widths: Vec<(PinId, PinDirection, f64)>,
    pub natural_height: f64,
    pub(super) properties: Vec<(String, f64)>,
    pub(super) input_heights: Vec<(PinId, f64)>,
    pub height: f64,
    pub pins_top: f64,
    /// Top offset of the Match cases block.
    pub cases_top: f64,
    pub(super) inputs: Vec<(PinId, f64)>,
    pub(super) outputs: Vec<(PinId, f64)>,
}

impl NodeLayout {
    pub fn new(def: &NodeDefinition, properties: &[(String, DataValue)]) -> Self {
        let pins_top = HEADER_H + BODY_PAD + property_rows(def, properties).len() as f64 * ROW_H;
        let row_y = |i: usize| pins_top + i as f64 * ROW_H + ROW_H / 2.0;
        let inputs: Vec<_> = pins(def, PinDirection::Input).enumerate().map(|(i, p)| (p.id, row_y(i))).collect();
        if def.type_name == "match" {
            // A split Match input retains its name/type editor row above its field rows.
            let mut row = 0;
            let mut previous_parent = None;
            let inputs: Vec<_> = pins(def, PinDirection::Input).map(|p| {
                let parent = ticknode_shared::split_field_parent(p.id).map(|(parent, _)| parent);
                if parent.is_some() && parent != previous_parent { row += 1; }
                previous_parent = parent;
                let at = (p.id, row_y(row));
                row += 1;
                at
            }).collect();
            let cases_top = pins_top + (row + 1) as f64 * ROW_H;
            let mut y = cases_top;
            let mut outputs = Vec::new();
            for case in match_cases(properties) {
                outputs.push((PinId(case.pin), y + ROW_H / 2.0));
                y += case_rows(&case) as f64 * ROW_H;
            }
            // "+ case" row with the Else output
            outputs.push((PinId(0), y + ROW_H / 2.0));
            y += ROW_H;
            return Self { natural_height: y + BODY_PAD, properties: vec![], input_heights: vec![], pin_widths: vec![], width: automatic_width(def, properties, &[]), height: y + BODY_PAD, pins_top, cases_top, inputs, outputs };
        }
        let outputs: Vec<_> = pins(def, PinDirection::Output).enumerate().map(|(i, p)| (p.id, row_y(i))).collect();
        let rows = inputs.len().max(outputs.len()).max(1);
        let height = pins_top + rows as f64 * ROW_H + BODY_PAD;
        Self { natural_height: height, properties: vec![], input_heights: vec![], pin_widths: vec![], width: automatic_width(def, properties, &[]), height, pins_top, cases_top: height, inputs, outputs }
    }

    /// Applies stored dimensions and distributes whole extra pixels over free text rows.
    pub fn sized(def: &NodeDefinition, properties: &[(String, DataValue)], size: Option<NodeSize>, connected: &[PinId]) -> Self {
        // Keep non-row properties (Match cases and adaptive metadata) for specialized layouts.
        let filtered: Vec<_> = properties.iter().filter(|(name, _)|
            !def.properties.iter().any(|p| p.name == *name && p.input.is_some_and(|pin| connected.contains(&pin)))
        ).cloned().collect();
        let properties = filtered.as_slice();
        let mut layout = Self::new(def, properties);
        layout.width = automatic_width(def, properties, connected);
        if let Some(size) = size.and_then(NodeSize::validated) {
            layout.width = size.width;
            layout.height = layout.height.max(size.height);
        }
        layout.pin_widths = row_widths(def, connected, properties, layout.width);
        if def.type_name == "match" { return layout; }
        let grow_props: Vec<_> = property_rows(def, properties).into_iter()
            .map(|(name, value)| (name.clone(), matches!(value, DataValue::String(_)) && !is_type_property(def, name)
                && def.properties.iter().find(|p| p.name == *name).is_none_or(|p| matches!(p.kind, PropertyKind::Text | PropertyKind::Multiline))))
            .collect();
        let grow_inputs: Vec<_> = pins(def, PinDirection::Input).map(|p|
            (p.id, p.pin_type == PinType::Data(DataType::String) && !connected.contains(&p.id) && !def.properties.iter().any(|prop| prop.input == Some(p.id)))).collect();
        let count = grow_props.iter().filter(|(_, grow)| *grow).count() + grow_inputs.iter().filter(|(_, grow)| *grow).count();
        let extra = (layout.height - layout.natural_height).floor();
        let share = if count == 0 { 0.0 } else { (extra / count as f64).floor() };
        let mut remaining = count;
        let mut addition = |grow: bool| {
            if !grow { return 0.0; }
            remaining -= 1;
            if remaining == 0 { extra - share * (count - 1) as f64 } else { share }
        };
        let mut shift = 0.0;
        for (name, grow) in grow_props {
            let more = addition(grow);
            layout.properties.push((name, ROW_H + more));
            shift += more;
        }
        layout.pins_top += shift;
        for (_, y) in &mut layout.outputs { *y += shift; }
        for ((id, grow), (_, y)) in grow_inputs.into_iter().zip(&mut layout.inputs) {
            *y += shift;
            let more = addition(grow);
            layout.input_heights.push((id, ROW_H + more));
            shift += more;
        }
        layout
    }

    pub fn pin_width(&self, pin: PinId, direction: PinDirection) -> f64 {
        self.pin_widths.iter().find(|(p, d, _)| *p == pin && *d == direction).map_or(self.width - 16.0, |(_, _, w)| *w)
    }

    pub fn property_height(&self, name: &str) -> f64 {
        self.properties.iter().find(|(n, _)| n == name).map_or(ROW_H, |(_, h)| *h)
    }

    pub fn pin_height(&self, pin: PinId, direction: PinDirection) -> f64 {
        if direction == PinDirection::Output { return ROW_H; }
        self.input_heights.iter().find(|(p, _)| *p == pin).map_or(ROW_H, |(_, h)| *h)
    }

    /// Existing pin anchor, or the old directional row clamped to the current layout.
    pub fn detached_anchor(&self, pos: Position, pin: PinId, direction: PinDirection, row: u32, loose: bool) -> (f64, f64) {
        let (rows, sign) = match direction {
            PinDirection::Input => (&self.inputs, -1.0),
            PinDirection::Output => (&self.outputs, 1.0),
        };
        let (x, y) = self.anchor(pos, pin, direction).unwrap_or_else(|| {
            let y = rows.get((row as usize).min(rows.len().saturating_sub(1)))
                .map_or(self.pins_top, |(_, y)| *y);
            (pos.x as f64 + if sign > 0.0 { self.width } else { 0.0 }, pos.y as f64 + y)
        });
        (x + if loose { sign * 8.0 * DU } else { 0.0 }, y)
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

/// Snaps a drag to design units while enforcing content and absolute bounds.
pub fn resize_size(origin: NodeSize, delta: (f64, f64), natural_height: f64) -> NodeSize {
    use ticknode_shared::{MIN_NODE_W, MAX_NODE_W, MAX_NODE_H};
    let snap = |v: f64| (v / DU).round() * DU;
    NodeSize {
        width: snap(origin.width + delta.0).clamp(MIN_NODE_W, MAX_NODE_W),
        height: snap(origin.height + delta.1).clamp(((natural_height / DU).ceil() * DU).min(MAX_NODE_H), MAX_NODE_H),
    }
}

#[cfg(test)]
mod tests;

mod width;
use width::{automatic_width, row_widths};

pub(crate) use width::compact_select_width;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod ha_tests;
