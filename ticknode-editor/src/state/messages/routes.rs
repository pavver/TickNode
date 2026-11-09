//! Idempotent layout updates and pruning on authoritative graph events.

use ticknode_shared::ServerMessage;
use crate::state::WireRoutes;

pub(super) fn applies(event: &ServerMessage) -> bool {
    matches!(event, ServerMessage::Init { .. } | ServerMessage::WireRouted(_) | ServerMessage::NodeRemoved(_)
        | ServerMessage::ExecDisconnected(_) | ServerMessage::DataDisconnected { .. } | ServerMessage::DataConnected(_))
}

pub(super) fn apply(all: &mut WireRoutes, event: &ServerMessage) {
    match event {
        ServerMessage::Init { wire_routes, .. } => {
            *all = wire_routes.iter().filter(|r| !r.points.is_empty()).map(|r| ((r.exec, r.connection), r.points.clone())).collect();
        }
        ServerMessage::WireRouted(route) => {
            let key = (route.exec, route.connection);
            if route.points.is_empty() { all.remove(&key); }
            else { all.insert(key, route.points.clone()); }
        }
        ServerMessage::NodeRemoved(id) => all.retain(|(_, c), _| c.src_node != *id && c.dst_node != *id),
        ServerMessage::ExecDisconnected(c) => { all.remove(&(true, *c)); }
        ServerMessage::DataDisconnected { dst_node, dst_pin } => {
            all.retain(|(exec, c), _| *exec || c.dst_node != *dst_node || c.dst_pin != *dst_pin);
        }
        ServerMessage::DataConnected(c) => {
            all.retain(|(exec, old), _| *exec || *old == *c || old.dst_node != c.dst_node || old.dst_pin != c.dst_pin);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
