use ticknode_shared::*;
use crate::{EngineError, TickEngine};

impl TickEngine {
    // --- Connections ---

    pub fn exec_connections(&self) -> &[ConnectionModel] {
        self.connections.exec()
    }

    pub fn data_connections(&self) -> &[ConnectionModel] {
        self.connections.data()
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
        self.connections.add_exec(c);
        Ok(())
    }

    pub fn disconnect_exec(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> bool {
        let c = ConnectionModel { src_node, src_pin, dst_node, dst_pin };
        self.connections.remove_exec(&c)
    }

    /// Whether `start` depends (via data input pins) on `target`.
    fn has_data_path(&self, start: NodeId, target: NodeId) -> bool {
        let mut stack = vec![start];
        let mut visited = std::collections::HashSet::new();
        while let Some(current) = stack.pop() {
            if current == target { return true; }
            if !visited.insert(current) { continue; }
            for c in self.connections.data_into(current) {
                stack.push(c.src_node);
            }
        }
        false
    }

    /// A data input pin has at most one source: a new connection replaces the previous one.
    pub fn connect_data(&mut self, src_node: NodeId, src_pin: PinId, dst_node: NodeId, dst_pin: PinId) -> Result<(), EngineError> {
        self.connect_data_inner(ConnectionModel { src_node, src_pin, dst_node, dst_pin }, true)
    }

    /// Snapshot restoration preserves persisted adaptive properties and confirmed contracts.
    pub(super) fn connect_saved_data(&mut self, c: ConnectionModel) -> Result<(), EngineError> {
        self.connect_data_inner(c, false)
    }

    fn connect_data_inner(&mut self, c: ConnectionModel, adapt: bool) -> Result<(), EngineError> {
        let ConnectionModel { src_node, src_pin, dst_node, dst_pin } = c;
        let source = self.find_pin(src_node, src_pin, PinDirection::Output)?;
        let target = self.find_pin(dst_node, dst_pin, PinDirection::Input)?;
        let other_inputs = self.connections.data().iter().any(|d| d.dst_node == dst_node && d.dst_pin != dst_pin);
        let adapts = adapt && !other_inputs && self.nodes.get(&dst_node).is_some_and(|n| n.adapts_on_connect(dst_pin))
            && matches!((&source, &target), (PinType::Data(_), PinType::Data(_)));
        if !adapts { self.validate_data_types(&c)?; }
        if self.has_data_path(src_node, dst_node) {
            return Err(EngineError::CycleDetected(dst_node, src_node));
        }
        if adapts && let PinType::Data(t) = source {
            self.adapt_node(dst_node, dst_pin, &t)?;
            // A node may decline the type (math accepts only numbers): the ordinary rules then apply.
            self.validate_data_types(&c)?;
        }
        self.connections.set_data(c);
        self.pure_cache.clear();
        Ok(())
    }

    pub fn disconnect_data(&mut self, dst_node: NodeId, dst_pin: PinId) -> bool {
        let removed = self.connections.remove_data_input(dst_node, dst_pin).is_some();
        self.pure_cache.clear();
        removed
    }
}
