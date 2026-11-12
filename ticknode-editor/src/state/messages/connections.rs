//! Idempotent connection updates received from the server.

use ticknode_shared::{ConnectionModel, NodeId, PinId};

pub(super) fn connect_exec(all: &mut Vec<ConnectionModel>, connection: ConnectionModel) {
    if !all.contains(&connection) {
        all.push(connection);
    }
}

pub(super) fn connect_data(all: &mut Vec<ConnectionModel>, connection: ConnectionModel) {
    disconnect_data(all, connection.dst_node, connection.dst_pin);
    all.push(connection);
}

pub(super) fn disconnect_data(all: &mut Vec<ConnectionModel>, node: NodeId, pin: PinId) {
    all.retain(|c| c.dst_node != node || c.dst_pin != pin);
}

#[cfg(test)]
mod tests;
