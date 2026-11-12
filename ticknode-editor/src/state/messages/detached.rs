//! Authoritative, idempotent detached wire updates.

use ticknode_shared::{DetachedConnection, ServerMessage};

pub(super) fn applies(event: &ServerMessage) -> bool {
    matches!(
        event,
        ServerMessage::Init { .. }
            | ServerMessage::Detached(_)
            | ServerMessage::DetachedRemoved(_)
            | ServerMessage::NodeRemoved(_)
    )
}

pub(super) fn apply(all: &mut Vec<DetachedConnection>, event: &ServerMessage) {
    match event {
        ServerMessage::Init {
            detached_connections,
            ..
        } => {
            all.clear();
            for d in detached_connections {
                if !all.contains(d) {
                    all.push(*d);
                }
            }
        }
        ServerMessage::Detached(d) => {
            if !all.contains(d) {
                all.push(*d);
            }
        }
        ServerMessage::DetachedRemoved(d) => all.retain(|x| x != d),
        ServerMessage::NodeRemoved(id) => {
            all.retain(|d| d.connection.src_node != *id && d.connection.dst_node != *id)
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
