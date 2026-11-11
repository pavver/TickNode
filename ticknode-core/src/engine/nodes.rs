use ticknode_shared::*;
use std::sync::Arc;
use crate::{EngineError, Node, RemovedConnections, TickEngine};

impl TickEngine {
    /// Current host-delay timers, grouped by owner node.
    pub fn delay_timers(&self) -> Vec<NodeTimer> {
        self.host_state.delay_timers_with_tasks().into_iter().map(|(_, timer)| timer).collect()
    }

    // --- Nodes ---

    pub fn add_node(&mut self, node: Box<dyn Node>) {
        let id = node.id();
        self.clear_node_error(id);
        self.invalidate_definition(id);
        self.confirmed_pins.entry(id).or_insert_with(|| pin_signature(&node.definition()));
        self.nodes.insert(id, node);
        self.update_roles(id);
    }

    pub fn is_disabled(&self, id: NodeId) -> bool {
        self.disabled.contains(&id)
    }

    pub fn set_disabled(&mut self, nodes: impl IntoIterator<Item = NodeId>, disabled: bool) {
        for id in nodes {
            if disabled {
                self.disabled.insert(id);
                self.pure_cache.remove(&id);
            } else {
                self.disabled.remove(&id);
            }
            self.update_roles(id);
        }
    }

    /// Keeps the per-tick node sets (event sources, plugin nodes kept live) in step with the node map.
    /// Called wherever a node is added, removed or changes its definition.
    fn update_roles(&mut self, id: NodeId) {
        let node = self.nodes.get(&id);
        let active = node.is_some() && !self.is_disabled(id);
        for (set, member) in [
            (&mut self.sources, active && node.unwrap().polls()),
            (&mut self.live_nodes, active && node.unwrap().needs_live()),
        ] {
            if member { set.insert(id); } else { set.remove(&id); }
        }
    }

    /// Creates a node via the registry and applies its properties. Input values (`in:<pin>`) are returned
    /// separately: they can only be set after properties because pins may depend on them.
    fn build_node(&self, id: NodeId, type_name: &str, properties: &[(String, DataValue)], validate: bool) -> Result<(Box<dyn Node>, Vec<(PinId, DataValue)>), EngineError> {
        let mut node = self.registry.create_node(type_name, id)
            .ok_or_else(|| EngineError::UnknownNodeType(type_name.to_string()))?;
        let mut inputs = Vec::new();
        for (name, value) in properties {
            match parse_input_value_property(name) {
                Some(pin) => inputs.push((pin, value.clone())),
                None if parse_split_pin_property(name).is_some() => {},
                None => {
                    let value = if validate { validated_property(node.as_ref(), name, value.clone())? } else { value.clone() };
                    node.set_property(name, value);
                },
            }
        }
        node.restored();
        Ok((node, inputs))
    }

    /// Creates a node via the registry and applies its properties.
    pub fn create_node(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(), EngineError> {
        self.create_node_impl(id, type_name, properties, false)
    }

    /// Creates an interactively configured node; validation finishes before insertion.
    pub fn create_node_interactive(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)]) -> Result<(), EngineError> {
        self.create_node_impl(id, type_name, properties, true)
    }

    fn create_node_impl(&mut self, id: NodeId, type_name: &str, properties: &[(String, DataValue)], validate: bool) -> Result<(), EngineError> {
        let (node, inputs) = self.build_node(id, type_name, properties, validate)?;
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

    pub(crate) fn definition_arc(&mut self, id: NodeId) -> Option<Arc<NodeDefinition>> {
        let node = self.nodes.get(&id)?;
        let revision = node.definition_revision();
        if let Some((cached_revision, definition)) = self.definitions.get(&id)
            && *cached_revision == revision
        {
            return Some(Arc::clone(definition));
        }
        let definition = Arc::new(self.effective_definition(id, node.definition()));
        self.definitions.insert(id, (revision, Arc::clone(&definition)));
        Some(definition)
    }

    pub(crate) fn invalidate_definition(&mut self, id: NodeId) {
        self.definitions.remove(&id);
    }

    pub(crate) fn clear_definitions(&mut self) {
        self.definitions.clear();
    }

    #[cfg(test)]
    pub(crate) fn fresh_node_definition(&self, id: NodeId) -> Option<NodeDefinition> {
        self.nodes.get(&id).map(|node| self.effective_definition(id, node.definition()))
    }

    /// Node model for persistence/editor (position and size are managed by the server, defaulted here).
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
            pins: self.confirmed_pins.get(&node_id).cloned(),
            size: None,
        })
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> RemovedConnections {
        self.clear_node_error(node_id);
        self.invalidate_definition(node_id);
        self.nodes.remove(&node_id);
        self.confirmed_pins.remove(&node_id);
        self.frozen.remove(&node_id);
        self.frozen_reported.remove(&node_id);
        self.disabled.remove(&node_id);
        self.detached.retain(|d| d.connection.src_node != node_id && d.connection.dst_node != node_id);
        self.update_roles(node_id);
        self.input_values.remove(&node_id);
        self.split_pins.remove(&node_id);
        self.required_reported.remove(&node_id);
        self.data_cache.remove(&node_id);
        self.pure_cache.remove(&node_id);
        self.execution_queue.retain(|(n, _)| *n != node_id);

        self.host_state.cancel_node_tasks(node_id);
        self.set_node_sheet(node_id, None);

        self.connections.retain(
            |c| c.src_node != node_id && c.dst_node != node_id,
            |c| c.src_node != node_id && c.dst_node != node_id,
        )
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
        let value = validated_property(node.as_ref(), name, value)?;
        node.set_property(name, value);
        self.finish_property_edit(node_id)
    }

    /// Adapts a node to an existing data pin and reconciles its definition, values and connections.
    pub fn adapt_node(&mut self, node_id: NodeId, pin: PinId, data_type: &DataType) -> Result<RemovedConnections, EngineError> {
        let node = self.nodes.get_mut(&node_id).ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        if !node.adapt_to(pin, data_type) { return Ok(RemovedConnections::default()); }
        self.finish_property_edit(node_id)
    }

    fn finish_property_edit(&mut self, node_id: NodeId) -> Result<RemovedConnections, EngineError> {
        self.invalidate_definition(node_id);
        self.pins_edited(node_id);
        self.update_roles(node_id);
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
        let invalid_exec: Vec<_> = self.connections.exec().iter()
            .filter(|c| touching(c) && self.validate_exec(c).is_err())
            .copied().collect();
        let invalid_data: Vec<_> = self.connections.data().iter()
            .filter(|c| touching(c) && self.validate_data_types(c).is_err())
            .copied().collect();
        self.connections.retain(
            |c| !invalid_exec.contains(c),
            |c| !invalid_data.contains(c),
        )
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
        self.clear_node_error(node_id);
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

/// Validate before invoking the infallible setter, including property-dependent schemas.
fn validated_property(node: &dyn Node, name: &str, value: DataValue) -> Result<DataValue, EngineError> {
    let value = match node.definition().properties.iter().find(|p| p.name == name) {
        Some(p) => normalize_property_value(&p.kind, &value).map_err(EngineError::InvalidProperty)?,
        None => value,
    };
    node.validate_property(name, &value).map_err(EngineError::InvalidProperty)?;
    Ok(value)
}
