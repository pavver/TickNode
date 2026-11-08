//! Pointer decisions and wire parsing without browser dependencies.

use super::{PinDirection, WireEnd, Selection, NodeId, PinId, ConnectionModel, ClientMessage, parse_node_id};

#[derive(Debug, PartialEq)]
pub(super) enum Press { Place, Cancel, Ignore, Continue }

pub(super) fn press(paste: bool, pointer_type: &str, button: i16) -> Press {
    if paste && button != 1 {
        return match button { 0 => Press::Place, 2 => Press::Cancel, _ => Press::Ignore };
    }
    if pointer_type == "mouse" && button != 0 && button != 1 { Press::Ignore } else { Press::Continue }
}

#[derive(Debug, PartialEq)]
pub(super) enum Gesture {
    Pan { moved: bool },
    Wire(WireEnd),
    Select(Selection),
    Toggle(NodeId),
    Nodes(NodeId),
    Box,
}

pub(super) fn gesture(button: i16, pointer_type: &str, additive: bool, pin: Option<WireEnd>, wire: Option<Selection>, node: Option<NodeId>) -> Gesture {
    match (button, pin, wire, node, pointer_type, additive) {
        (1, ..) => Gesture::Pan { moved: true },
        (_, Some(pin), ..) => Gesture::Wire(pin),
        (_, _, Some(wire), ..) => Gesture::Select(wire),
        (_, _, _, Some(id), _, true) => Gesture::Toggle(id),
        (_, _, _, Some(id), _, false) => Gesture::Nodes(id),
        (_, _, _, _, "touch", _) => Gesture::Pan { moved: false },
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
