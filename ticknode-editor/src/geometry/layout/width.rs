//! Content widths use the bundled Cascadia monospace font (1200/2048 em advance).
use super::*;
use crate::{i18n, panels::property_label};

pub(super) const CHAR_WIDTH: f64 = 12.0 * 1200.0 / 2048.0;
pub(super) const FIELD_MIN: f64 = 18.0 * DU;
const SELECT_CHROME: f64 = 24.0;

pub(super) fn text_width(text: &str) -> f64 {
    text.chars().count() as f64 * CHAR_WIDTH
}

fn duration_width(signed: bool, wall_only: bool) -> f64 {
    let unit = crate::inline::duration::units::Unit::available(wall_only)
        .map(|u| text_width(&u.label()))
        .fold(0.0, f64::max);
    FIELD_MIN + unit + SELECT_CHROME + 2.0 + if signed { 22.0 } else { 0.0 }
}

/// Native select chrome plus the selected text, capped like the compact CSS control.
pub(crate) fn compact_select_width(label: &str) -> f64 {
    (text_width(label) + SELECT_CHROME).min(40.0 * DU)
}

pub(super) fn header_width(def: &NodeDefinition) -> f64 {
    let title = text_width(&i18n::node_title(def)) * 13.0 / 12.0;
    let badge = text_width(&crate::categories::category_label(&def.category)) * 11.0 / 12.0;
    let run = if def.pins.iter().any(|p| p.pin_type == PinType::Exec) {
        26.0
    } else {
        0.0
    };
    title + badge + 26.0 + run
}

fn pin_width(
    def: &NodeDefinition,
    pin: &PinDefinition,
    connected: &[PinId],
    properties: &[(String, DataValue)],
) -> f64 {
    let label = i18n::node_pin_label(&def.type_name, &pin.name);
    let field = if pin.direction == PinDirection::Input
        && !connected.contains(&pin.id)
        && !def.properties.iter().any(|p| p.input == Some(pin.id))
    {
        match &pin.pin_type {
            PinType::Data(DataType::Duration) => duration_width(false, false),
            PinType::Data(DataType::Bool) => 48.0,
            PinType::Data(DataType::Named(name)) => ticknode_shared::quantity::kind(name)
                .map_or_else(
                    || {
                        let key = ticknode_shared::input_value_property(pin.id);
                        let selected = properties
                            .iter()
                            .find(|(n, _)| *n == key)
                            .and_then(|(_, v)| {
                                if let DataValue::String(s) = v {
                                    Some(s.as_str())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or("");
                        i18n::enum_selected_label(name, selected)
                            .map_or(0.0, |label| compact_select_width(&label))
                    },
                    |q| {
                        FIELD_MIN
                            + q.units
                                .iter()
                                .map(|u| {
                                    text_width(
                                        &i18n::unit_symbol(&u.id)
                                            .unwrap_or_else(|| u.symbol.clone()),
                                    )
                                })
                                .fold(0.0, f64::max)
                            + SELECT_CHROME
                            + 2.0
                    },
                ),
            PinType::Data(t) if ticknode_shared::text::is_text_type(t) => FIELD_MIN,
            _ => 0.0,
        }
    } else {
        0.0
    };
    let is_field = ticknode_shared::split_field_parent(pin.id).is_some();
    text_width(&label)
        + field
        + 24.0
        + if is_field || i18n::pin_can_split(pin) {
            22.0
        } else {
            0.0
        }
        + if is_field { 5.0 } else { 0.0 }
}

pub(super) fn automatic_width(
    def: &NodeDefinition,
    properties: &[(String, DataValue)],
    connected: &[PinId],
) -> f64 {
    let mut width = node_width(&def.type_name).max(header_width(def));
    for (name, value) in property_rows_connected(def, properties, connected) {
        let described = def.properties.iter().find(|p| p.name == *name);
        let label = described.map_or_else(
            || property_label(name),
            |p| i18n::property_title(&def.type_name, p),
        );
        let field = match described.map(|p| &p.kind) {
            Some(PropertyKind::Select(options)) => {
                let selected = match value {
                    DataValue::String(s) => s.as_str(),
                    _ => "",
                };
                let label = options.iter().find(|(v, _)| v == selected).map_or_else(
                    || selected.to_string(),
                    |(v, label)| i18n::property_option_label(&def.type_name, name, v, label),
                );
                compact_select_width(&label)
            }
            Some(PropertyKind::MultiSelect(options)) => {
                options
                    .iter()
                    .map(|(v, label)| {
                        text_width(&i18n::property_option_short_label(
                            &def.type_name,
                            name,
                            v,
                            label,
                        )) * 11.0
                            / 12.0
                            + 10.0
                    })
                    .sum::<f64>()
                    + options.len().saturating_sub(1) as f64 * 2.0
            }
            Some(PropertyKind::Duration { signed }) => duration_width(*signed, true),
            _ if matches!(value, DataValue::Bool(_)) => 8.5 * DU,
            _ => FIELD_MIN,
        };
        width = width.max(text_width(&label) + field + 28.0);
    }
    let inputs: Vec<_> = pins(def, PinDirection::Input).collect();
    let outputs: Vec<_> = pins(def, PinDirection::Output).collect();
    for row in 0..inputs.len().max(outputs.len()) {
        width = width.max(
            inputs
                .get(row)
                .map_or(0.0, |p| pin_width(def, p, connected, properties))
                + outputs
                    .get(row)
                    .map_or(0.0, |p| pin_width(def, p, connected, properties))
                + 16.0,
        );
    }
    ((width / DU).ceil() * DU).clamp(node_width(&def.type_name), 2.0 * NODE_WIDTH)
}

/// Allocate each directional row its content width; shrink both proportionally at the width cap.
pub(super) fn row_widths(
    def: &NodeDefinition,
    connected: &[PinId],
    properties: &[(String, DataValue)],
    width: f64,
) -> Vec<(PinId, PinDirection, f64)> {
    let inputs: Vec<_> = pins(def, PinDirection::Input).collect();
    let outputs: Vec<_> = pins(def, PinDirection::Output).collect();
    let mut widths = Vec::new();
    for row in 0..inputs.len().max(outputs.len()) {
        let a = inputs
            .get(row)
            .map_or(0.0, |p| pin_width(def, p, connected, properties));
        let b = outputs
            .get(row)
            .map_or(0.0, |p| pin_width(def, p, connected, properties));
        let scale = ((width - 16.0) / (a + b)).min(1.0);
        if let Some(p) = inputs.get(row) {
            widths.push((p.id, p.direction, (width - 16.0 - b * scale).max(0.0)));
        }
        if let Some(p) = outputs.get(row) {
            widths.push((p.id, p.direction, b * scale));
        }
    }
    widths
}
