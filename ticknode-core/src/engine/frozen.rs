use ticknode_shared::*;
use crate::{EngineError, RemovedConnections, TickEngine};

impl TickEngine {
    pub fn frozen_changes(&self, id: NodeId) -> &[PinChange] {
        self.frozen.get(&id).map_or(&[], Vec::as_slice)
    }

    pub fn detached_connections(&self) -> &[DetachedConnection] { &self.detached }

    pub fn remove_detached(&mut self, connection: &DetachedConnection) -> bool {
        let before = self.detached.len();
        self.detached.retain(|d| d != connection);
        self.detached.len() != before
    }

    pub fn confirm_pins(&mut self, id: NodeId) -> Result<bool, EngineError> {
        let node = self.nodes.get(&id).ok_or(EngineError::NodeNotFound(id))?;
        if !self.frozen.contains_key(&id) { return Ok(false); }
        if let Some(reason) = node.unavailable() { return Err(EngineError::PluginUnavailable(reason)); }
        self.accept_current_pins(id);
        self.clear_node_error(id);
        Ok(true)
    }

    pub(crate) fn accept_current_pins(&mut self, id: NodeId) {
        if let Some(node) = self.nodes.get(&id) {
            self.confirmed_pins.insert(id, pin_signature(&node.definition()));
        }
        self.frozen.remove(&id);
        self.frozen_reported.remove(&id);
        self.required_reported.remove(&id);
        self.pure_cache.clear();
    }

    /// After a user edit changes pins: an unfrozen node accepts them, a frozen node stays frozen
    /// and its changes are recomputed against the contract the user has not confirmed yet.
    pub(crate) fn pins_edited(&mut self, id: NodeId) {
        self.clear_node_error(id);
        match self.frozen.contains_key(&id).then(|| self.confirmed_pins.get(&id).cloned()).flatten() {
            Some(confirmed) => self.restore_pins(id, Some(&confirmed)),
            None => self.accept_current_pins(id),
        }
    }

    /// Stubs retain their last confirmed contract without comparing an inferred definition.
    pub(crate) fn restore_pins(&mut self, id: NodeId, stored: Option<&[PinSignature]>) {
        let Some(node) = self.nodes.get(&id) else { return };
        if node.unavailable().is_some() {
            if let Some(pins) = stored { self.confirmed_pins.insert(id, pins.to_vec()); }
            return;
        }
        let current = pin_signature(&node.definition());
        let changes = stored.map_or_else(Vec::new, |old| pin_changes(old, &current).0);
        if changes.is_empty() {
            if self.frozen.contains_key(&id) { self.accept_current_pins(id); }
            else { self.confirmed_pins.insert(id, current); }
        } else {
            self.confirmed_pins.insert(id, stored.expect("changes require an old signature").to_vec());
            if self.frozen.get(&id) != Some(&changes) { self.frozen_reported.remove(&id); }
            self.frozen.insert(id, changes);
            self.data_cache.remove(&id);
            self.pure_cache.clear();
        }
    }

    pub(crate) fn skip_frozen(&mut self, id: NodeId) -> bool {
        if !self.frozen.contains_key(&id) { return false; }
        if self.frozen_reported.insert(id) {
            self.host_state.error(id, UiMessage::new("error-node-frozen", "Node is frozen: its pins changed; confirm it in the editor"), "Node is frozen: its pins changed; confirm it in the editor".into());
        }
        true
    }

    /// Only pin-contract failures become detached wires; missing nodes and cycles still warn.
    pub(crate) fn detach_invalid(&mut self, exec: bool, connection: ConnectionModel, error: &EngineError) -> bool {
        if !matches!(error, EngineError::InvalidPinConnection | EngineError::TypeMismatch(..))
            || !self.has_node(connection.src_node) || !self.has_node(connection.dst_node) { return false; }
        let endpoint = |id, pin, direction| {
            let base = split_field_parent(pin).map_or(pin, |(parent, _)| parent);
            let old = self.confirmed_pins.get(&id).into_iter().flatten()
                .filter(|p| p.direction == direction).collect::<Vec<_>>();
            let row = old.iter().position(|p| p.id == base).unwrap_or(0) as u32;
            let actual = self.find_pin(id, pin, direction).ok();
            let expected = old.iter().find(|p| p.id == base).map(|p| &p.kind);
            let changed = match actual.as_ref() {
                None => true,
                Some(kind) => matches!(kind, PinType::Exec) != exec || (!is_virtual_pin(pin) && expected.is_some_and(|old| old != kind)),
            };
            (changed, row)
        };
        let (target_changed, target_row) = endpoint(connection.dst_node, connection.dst_pin, PinDirection::Input);
        let (source_changed, source_row) = endpoint(connection.src_node, connection.src_pin, PinDirection::Output);
        let (end, row) = if target_changed || !source_changed { (DetachedEnd::Target, target_row) }
            else { (DetachedEnd::Source, source_row) };
        let detached = DetachedConnection { exec, connection, end, row };
        if !self.detached.contains(&detached) { self.detached.push(detached); }
        true
    }

    pub(crate) fn detach_pruned(&mut self, removed: RemovedConnections) -> Vec<DetachedConnection> {
        let before = self.detached.clone();
        for (exec, list) in [(true, removed.exec), (false, removed.data)] {
            for c in list { self.detach_invalid(exec, c, &EngineError::InvalidPinConnection); }
        }
        self.detached.iter().filter(|d| !before.contains(d)).copied().collect()
    }

    pub(crate) fn restore_detached(&mut self, detached: &[DetachedConnection]) {
        for d in detached {
            if self.has_node(d.connection.src_node) && self.has_node(d.connection.dst_node) && !self.detached.contains(d) {
                self.detached.push(*d);
            }
        }
    }
}

#[cfg(test)]
mod tests;
