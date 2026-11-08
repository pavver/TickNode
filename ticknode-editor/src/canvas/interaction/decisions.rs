//! Pointer decisions and wire parsing without browser dependencies.

use super::{PinDirection, WireEnd, Selection, NodeId, PinId, ConnectionModel, ClientMessage, parse_node_id};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeClick { CopySettings, PasteSettings, ToggleSelect, Select, Ignore }

pub(super) fn node_click_action(button: i16, shift: bool, ctrl: bool, copied_type: Option<&str>, node_type: &str) -> NodeClick {
    match (button, shift, ctrl) {
        (2, true, _) => NodeClick::CopySettings,
        (0, _, true) => NodeClick::ToggleSelect,
        (0, true, false) if copied_type == Some(node_type) => NodeClick::PasteSettings,
        (0, true, false) => NodeClick::ToggleSelect,
        (0, false, false) => NodeClick::Select,
        _ => NodeClick::Ignore,
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum Press { Place, Cancel, Ignore, Continue }

pub(super) fn press(paste: bool, pointer_type: &str, button: i16) -> Press {
    if paste && button != 1 {
        return match button { 0 => Press::Place, 2 => Press::Cancel, _ => Press::Ignore };
    }
    if pointer_type == "mouse" && !(0..=2).contains(&button) { Press::Ignore } else { Press::Continue }
}

#[derive(Debug, PartialEq)]
pub(super) enum Gesture {
    Pan { moved: bool },
    Wire(WireEnd),
    Select(Selection),
    Node(NodeId),
    Box,
    Ignore,
}

pub(super) fn gesture(button: i16, pointer_type: &str, pin: Option<WireEnd>, wire: Option<Selection>, node: Option<NodeId>) -> Gesture {
    match (button, pin, wire, node, pointer_type) {
        (1, ..) => Gesture::Pan { moved: true },
        (2, _, _, Some(id), _) => Gesture::Node(id),
        (2, ..) => Gesture::Ignore,
        (_, Some(pin), ..) => Gesture::Wire(pin),
        (_, _, Some(wire), ..) => Gesture::Select(wire),
        (_, _, _, Some(id), _) => Gesture::Node(id),
        (_, _, _, _, "touch") => Gesture::Pan { moved: false },
        _ => Gesture::Box,
    }
}

pub(super) fn wire_selection(src: &str, src_pin: &str, dst: &str, dst_pin: &str, kind: &str) -> Option<Selection> {
    let connection = ConnectionModel {
        src_node: parse_node_id(src)?, src_pin: PinId(src_pin.parse().ok()?),
        dst_node: parse_node_id(dst)?, dst_pin: PinId(dst_pin.parse().ok()?),
    };
    Some(if kind == "exec" { Selection::Exec(connection) } else { Selection::Data(connection) })
}

pub(super) fn wire_command(from: WireEnd, to: WireEnd) -> Result<Option<ClientMessage>, &'static str> {
    if to.node == from.node && to.pin == from.pin { return Ok(None); }
    if to.dir == from.dir { return Err("[ERROR] Only an output can be connected to an input"); }
    if to.exec != from.exec { return Err("[ERROR] An exec pin can only be connected to an exec pin"); }
    let (out, inp) = if from.dir == PinDirection::Output { (from, to) } else { (to, from) };
    let c = ConnectionModel { src_node: out.node, src_pin: out.pin, dst_node: inp.node, dst_pin: inp.pin };
    Ok(Some(if from.exec { ClientMessage::ConnectExec(c) } else { ClientMessage::ConnectData(c) }))
}

#[cfg(test)]
mod tests;
