use ticknode_shared::*;
use crate::{EngineError, Node, RemovedConnections, TickEngine};

impl TickEngine {
    // --- Nodes ---

    pub fn add_node(&mut self, node: Box<dyn Node>) {
        self.nodes.insert(node.id(), node);
    }

    /// Creates a node via the registry and applies its properties. Input values (`in:<pin>`) are returned
    /// separately: they can only be set after properties because pins may depend on them.
    fn build_node(&self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(Box<dyn Node>, Vec<(PinId, DataValue)>), EngineError> {
        let mut node = self.registry.create_node(type_name, id)
            .ok_or_else(|| EngineError::UnknownNodeType(type_name.to_string()))?;
        let mut inputs = Vec::new();
        for (name, value) in properties {
            match parse_input_value_property(name) {
                Some(pin) => inputs.push((pin, value.clone())),
                None if parse_split_pin_property(name).is_some() => {},
                None => node.set_property(name, value.clone()),
            }
        }
        node.restored();
        Ok((node, inputs))
    }

    /// Creates a node via the registry and applies its properties.
    pub fn create_node(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(), EngineError> {
        let (node, inputs) = self.build_node(id, type_name, properties)?;
        self.add_node(node);
        self.split_pins.remove(&id);
        for (name, value) in properties {
            if let Some(pin) = parse_split_pin_property(name) && *value == DataValue::Bool(true) {
                self.split_pins.entry(id).or_default().insert(pin.0);
            }
        }
        // Pins depend on properties, so input values are applied afterward
        for (pin, value) in inputs {
            let _ = self.set_input_value(id, pin, Some(value));
        }
        Ok(())
    }

    /// Why the node is unavailable (stub); `None` if it is a real node.
    pub fn node_unavailable(&self, node_id: NodeId) -> Option<UiMessage> {
        self.nodes.get(&node_id)?.unavailable()
    }

    pub fn has_node(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    pub fn node_ids(&self) -> Vec<NodeId> {
        self.nodes.keys().copied().collect()
    }

    pub fn node_definition(&self, node_id: NodeId) -> Option<NodeDefinition> {
        self.nodes.get(&node_id).map(|n| self.effective_definition(node_id, n.definition()))
    }

    /// Node model for persistence/editor (position is managed by the server, defaulted here).
    pub fn node_model(&self, node_id: NodeId) -> Option<NodeModel> {
        self.nodes.get(&node_id).map(|n| NodeModel {
            id: node_id,
            type_name: n.definition().type_name,
            properties: n.properties().into_iter()
                .chain(self.split_pins.get(&node_id).into_iter().flatten()
                    .map(|pin| (split_pin_property(PinId(*pin)), DataValue::Bool(true))))
                .chain(self.input_values.get(&node_id).into_iter().flatten()
                    .map(|(pin, v)| (input_value_property(PinId(*pin)), v.clone())))
                .collect(),
            position: Position::default(),
        })
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> RemovedConnections {
        self.nodes.remove(&node_id);
        self.input_values.remove(&node_id);
        self.split_pins.remove(&node_id);
        self.required_reported.remove(&node_id);
        self.data_cache.remove(&node_id);
        self.pure_cache.remove(&node_id);
        self.execution_queue.retain(|(n, _)| *n != node_id);

        self.host_state.cancel_node_tasks(node_id);

        let mut removed = RemovedConnections::default();
        self.exec_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.exec.push(*c); }
            keep
        });
        self.data_connections.retain(|c| {
            let keep = c.src_node != node_id && c.dst_node != node_id;
            if !keep { removed.data.push(*c); }
            keep
        });
        removed
    }

    /// Sets a node property. If pins changed as a result, invalid connections are pruned.
    pub fn set_node_property(&mut self, node_id: NodeId, name: &str, value: DataValue) -> Result<RemovedConnections, EngineError> {
        if let Some(pin) = parse_split_pin_property(name) {
            let DataValue::Bool(split) = value else { return Err(EngineError::InvalidPinConnection) };
            return self.set_pin_split(node_id, pin, split);
        }
        if let Some(pin) = parse_input_value_property(name) {
            self.set_input_value(node_id, pin, Some(value))?;
            return Ok(RemovedConnections::default());
        }
        let node = self.nodes.get_mut(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        node.set_property(name, value);
        self.pure_cache.remove(&node_id);
        self.data_cache.remove(&node_id);

        // Input values whose pins disappeared or changed type are no longer valid
        let definition = self.node_definition(node_id).expect("node exists");
        if let Some(values) = self.input_values.get_mut(&node_id) {
            values.retain(|pin, v| input_pin_type(&definition, PinId(*pin)).is_some_and(|t| input_value_fits(&self.types, &t, v)));
        }

        Ok(self.prune_connections(|c| c.src_node == node_id || c.dst_node == node_id))
    }

    /// Prunes connections touching selected nodes that have become invalid (pins changed).
    pub(crate) fn prune_connections(&mut self, touching: impl Fn(&ConnectionModel) -> bool) -> RemovedConnections {
        let invalid_exec: Vec<_> = self.exec_connections.iter()
            .filter(|c| touching(c) && self.validate_exec(c).is_err())
            .copied().collect();
        let invalid_data: Vec<_> = self.data_connections.iter()
            .filter(|c| touching(c) && self.validate_data_types(c).is_err())
            .copied().collect();
        self.exec_connections.retain(|c| !invalid_exec.contains(c));
        self.data_connections.retain(|c| !invalid_data.contains(c));
        RemovedConnections { exec: invalid_exec, data: invalid_data }
    }

    /// Value of an unconnected data input (`None` clears it). Effective while nothing is connected to the pin.
    pub fn set_input_value(&mut self, node_id: NodeId, pin: PinId, value: Option<DataValue>) -> Result<(), EngineError> {
        let node = self.nodes.get(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let definition = self.node_definition(node_id).expect("node exists");
        let values = self.input_values.entry(node_id).or_default();
        match value {
            Some(value) => {
                let pin_type = input_pin_type(&definition, pin).ok_or(EngineError::InvalidPinConnection)?;
                if !input_value_fits(&self.types, &pin_type, &value) {
                    return Err(EngineError::TypeMismatch(value.get_type(), pin_type));
                }
                values.insert(pin.0, value);
            }
            None => {
                values.remove(&pin.0);
            }
        }
        self.pure_cache.clear();
        Ok(())
    }
}

/// Data type of an input pin (`None` if no such pin exists or it is an exec pin).
pub(crate) fn input_pin_type(definition: &NodeDefinition, pin: PinId) -> Option<DataType> {
    definition.pins.iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Input)
        .and_then(|p| match &p.pin_type {
            PinType::Data(t) => Some(t.clone()),
            PinType::Exec => None,
        })
}
