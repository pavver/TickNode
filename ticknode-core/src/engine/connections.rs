use ticknode_shared::*;
use crate::{EngineError, TickEngine};

impl TickEngine {
    // --- Connections ---

    pub fn exec_connections(&self) -> &[ConnectionModel] {
        &self.exec_connections
    }

    pub fn data_connections(&self) -> &[ConnectionModel] {
        &self.data_connections
    }

    pub(super) fn find_pin(&self, node_id: NodeId, pin_id: PinId, direction: PinDirection) -> Result<PinType, EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        self.effective_definition(node_id, node.definition()).pins.into_iter()
            .find(|p| p.id == pin_id && p.direction == direction)
            .map(|p| p.pin_type)
            .ok_or(EngineError::InvalidPinConnection)
    }

    pub(super) fn validate_exec(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Exec, PinType::Exec) => Ok(()),
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    pub(super) fn validate_data_types(&self, c: &ConnectionModel) -> Result<(), EngineError> {
        match (self.find_pin(c.src_node, c.src_pin, PinDirection::Output)?, self.find_pin(c.dst_node, c.dst_pin, PinDirection::Input)?) {
            (PinType::Data(s_type), PinType::Data(d_type)) => {
                if self.types.compatible(&s_type, &d_type) || self.types.convertible(&s_type, &d_type).is_some() {
                    Ok(())
                } else {
                    Err(EngineError::TypeMismatch(s_type, d_type))
                }
            }
            _ => Err(EngineError::InvalidPinConnection),
        }
    }

    pub fn connect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_exec(&c)?;
        if !self.exec_connections.contains(&c) {
            self.exec_connections.push(c);
        }
        Ok(())
    }

    pub fn disconnect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> bool {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        let before = self.exec_connections.len();
        self.exec_connections.retain(|x| *x != c);
        before != self.exec_connections.len()
    }

    /// Whether `start` depends (via data input pins) on `target`.
    fn has_data_path(&self, start: NodeId, target: NodeId) -> bool {
        let mut stack = vec![start];
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current == target { return true; }
            if !visited.insert(current) { continue; }
            for c in &self.data_connections {
                if c.dst_node == current { stack.push(c.src_node); }
            }
        }
        false
    }

    /// A data input pin has at most one source: a new connection replaces the previous one.
    pub fn connect_data(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.validate_data_types(&c)?;
        if self.has_data_path(src_node, dst_node) {
            return Err(EngineError::CycleDetected(dst_node, src_node));
        }
        self.data_connections.retain(|x| x.dst_node != dst_node || x.dst_pin != dst_pin);
        self.data_connections.push(c);
        self.pure_cache.clear();
        Ok(())
    }

    pub fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId) -> bool {
        let before = self.data_connections.len();
        self.data_connections.retain(|c| c.dst_node != dst_node || c.dst_pin != dst_pin);
        self.pure_cache.clear();
        before != self.data_connections.len()
    }
}
