use crate::{EngineError, Node, RemovedConnections, TickEngine};
use std::sync::Arc;
use ticknode_shared::*;

mod inputs;
pub(crate) use inputs::*;

impl TickEngine {
    /// Current host-delay timers, grouped by owner node.
    pub fn delay_timers(&self) -> Vec<NodeTimer> {
        self.host_state
            .delay_timers_with_tasks()
            .into_iter()
            .map(|(_, timer)| timer)
            .collect()
    }

    // --- Nodes ---

    pub fn add_node(&mut self, node: Box<dyn Node>) {
        let id = node.id();
        self.clear_node_error(id);
        self.invalidate_definition(id);
        self.confirmed_pins
            .entry(id)
            .or_insert_with(|| pin_signature(&node.definition()));
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
            if member {
                set.insert(id);
            } else {
                set.remove(&id);
            }
        }
    }

    /// Creates a node via the registry and applies its properties. Input values and variable bindings
    /// are returned separately: they can only be set after properties because pins may depend on them.
    fn build_node(
        &self,
        id: NodeId,
        type_name: &str,
        properties: &[(String, DataValue)],
        validate: bool,
    ) -> Result<(Box<dyn Node>, Vec<PendingInput>), EngineError> {
        let mut node = self
            .registry
            .create_node(type_name, id)
            .ok_or_else(|| EngineError::UnknownNodeType(type_name.to_string()))?;
        node.types_changed(&self.types);
        let mut inputs = Vec::new();
        for (name, value) in properties {
            if let Some(pin) = parse_input_value_property(name) {
                inputs.push(PendingInput::Value(pin, value.clone()));
            } else if let Some(pin) = input_variable_pin(name) {
                if let DataValue::String(s) = value
                    && let Some(var_ref) = VariableRef::parse(s)
                {
                    inputs.push(PendingInput::Variable(pin, var_ref));
                }
            } else if parse_split_pin_property(name).is_some() {
                // split pins are handled separately after node creation
            } else {
                let value = if validate {
                    validated_property(node.as_ref(), name, value.clone())?
                } else {
                    value.clone()
                };
                node.set_property(name, value);
            }
        }
        node.restored();
        node.types_changed(&self.types);
        Ok((node, inputs))
    }

    /// Creates a node via the registry and applies its properties.
    pub fn create_node(
        &mut self,
        id: NodeId,
        type_name: &str,
        properties: &[(String, DataValue)],
    ) -> Result<(), EngineError> {
        self.create_node_impl(id, type_name, properties, false)
    }

    /// Creates an interactively configured node; validation finishes before insertion.
    pub fn create_node_interactive(
        &mut self,
        id: NodeId,
        type_name: &str,
        properties: &[(String, DataValue)],
    ) -> Result<(), EngineError> {
        self.create_node_impl(id, type_name, properties, true)
    }

    fn create_node_impl(
        &mut self,
        id: NodeId,
        type_name: &str,
        properties: &[(String, DataValue)],
        validate: bool,
    ) -> Result<(), EngineError> {
        let (node, inputs) = self.build_node(id, type_name, properties, validate)?;
        self.add_node(node);
        self.split_pins.remove(&id);
        for (name, value) in properties {
            if let Some(pin) = parse_split_pin_property(name)
                && *value == DataValue::Bool(true)
            {
                self.split_pins.entry(id).or_default().insert(pin.0);
            }
        }
        // Pins depend on properties, so input values and bindings are applied afterward
        for input in inputs {
            match input {
                PendingInput::Value(pin, value) => {
                    let _ = self.set_input_value(id, pin, Some(value));
                }
                PendingInput::Variable(pin, var_ref) => {
                    let _ = self.set_input_variable(id, pin, Some(var_ref));
                }
            }
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
        self.nodes
            .get(&node_id)
            .map(|n| self.effective_definition(node_id, n.definition()))
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
        self.definitions
            .insert(id, (revision, Arc::clone(&definition)));
        Some(definition)
    }

    pub(crate) fn invalidate_definition(&mut self, id: NodeId) {
        self.definitions.remove(&id);
    }

    pub(crate) fn clear_definitions(&mut self) {
        self.definitions.clear();
    }

    pub(crate) fn notify_types_changed(&mut self) {
        self.clear_definitions();
        for node in self.nodes.values_mut() {
            node.types_changed(&self.types);
        }
        let ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        for id in ids {
            let confirmed = self.confirmed_pins.get(&id).cloned();
            self.restore_pins(id, confirmed.as_deref());
            self.update_roles(id);
        }
    }

    #[cfg(test)]
    pub(crate) fn fresh_node_definition(&self, id: NodeId) -> Option<NodeDefinition> {
        self.nodes
            .get(&id)
            .map(|node| self.effective_definition(id, node.definition()))
    }

    /// Node model for persistence/editor (position and size are managed by the server, defaulted here).
    pub fn node_model(&self, node_id: NodeId) -> Option<NodeModel> {
        self.nodes.get(&node_id).map(|n| NodeModel {
            id: node_id,
            type_name: n.definition().type_name,
            properties: n
                .properties()
                .into_iter()
                .chain(
                    self.split_pins
                        .get(&node_id)
                        .into_iter()
                        .flatten()
                        .map(|pin| (split_pin_property(PinId(*pin)), DataValue::Bool(true))),
                )
                .chain(
                    self.input_values
                        .get(&node_id)
                        .into_iter()
                        .flatten()
                        .map(|(pin, v)| (input_value_property(PinId(*pin)), v.clone())),
                )
                .chain(
                    self.input_variables
                        .get(&node_id)
                        .into_iter()
                        .flatten()
                        .map(|(pin, v)| {
                            (
                                input_variable_property(PinId(*pin)),
                                DataValue::String(v.to_str()),
                            )
                        }),
                )
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
        self.detached
            .retain(|d| d.connection.src_node != node_id && d.connection.dst_node != node_id);
        self.update_roles(node_id);
        self.input_values.remove(&node_id);
        self.input_variables.remove(&node_id);
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
    pub fn set_node_property(
        &mut self,
        node_id: NodeId,
        name: &str,
        value: DataValue,
    ) -> Result<RemovedConnections, EngineError> {
        if let Some(pin) = parse_split_pin_property(name) {
            let DataValue::Bool(split) = value else {
                return Err(EngineError::InvalidPinConnection);
            };
            return self.set_pin_split(node_id, pin, split);
        }
        if let Some(pin) = parse_input_value_property(name) {
            self.set_input_value(node_id, pin, Some(value))?;
            return Ok(RemovedConnections::default());
        }
        if let Some(pin) = input_variable_pin(name) {
            let variable = match value {
                DataValue::String(ref s) => VariableRef::parse(s).ok_or_else(|| {
                    EngineError::InvalidProperty(
                        UiMessage::new(
                            "error-invalid-variable-ref",
                            format!("Invalid variable reference: {s}"),
                        )
                        .arg("reference", s),
                    )
                })?,
                _ => {
                    return Err(EngineError::InvalidProperty(
                        UiMessage::new(
                            "error-invalid-property",
                            format!("Invalid property: {name}"),
                        )
                        .arg("property", name),
                    ));
                }
            };
            self.set_input_variable(node_id, pin, Some(variable))?;
            return Ok(RemovedConnections::default());
        }
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let value = validated_property(node.as_ref(), name, value)?;
        node.set_property(name, value);
        self.finish_property_edit(node_id)
    }

    /// Adapts a node to an existing data pin and reconciles its definition, values and connections.
    pub fn adapt_node(
        &mut self,
        node_id: NodeId,
        pin: PinId,
        data_type: &DataType,
    ) -> Result<RemovedConnections, EngineError> {
        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        if !node.adapt_to(pin, data_type) {
            return Ok(RemovedConnections::default());
        }
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
            values.retain(|pin, v| {
                input_pin_type(&definition, PinId(*pin))
                    .is_some_and(|t| input_value_fits(&self.types, &t, v))
            });
        }
        if let Some(vars) = self.input_variables.get_mut(&node_id) {
            vars.retain(|pin, _| input_pin_type(&definition, PinId(*pin)).is_some());
        }

        Ok(self.prune_connections(|c| c.src_node == node_id || c.dst_node == node_id))
    }

    /// Prunes connections touching selected nodes that have become invalid (pins changed).
    pub(crate) fn prune_connections(
        &mut self,
        touching: impl Fn(&ConnectionModel) -> bool,
    ) -> RemovedConnections {
        let invalid_exec: Vec<_> = self
            .connections
            .exec()
            .iter()
            .filter(|c| touching(c) && self.validate_exec(c).is_err())
            .copied()
            .collect();
        let invalid_data: Vec<_> = self
            .connections
            .data()
            .iter()
            .filter(|c| touching(c) && self.validate_data_types(c).is_err())
            .copied()
            .collect();
        self.connections
            .retain(|c| !invalid_exec.contains(c), |c| !invalid_data.contains(c))
    }
}

/// Validate before invoking the infallible setter, including property-dependent schemas.
fn validated_property(
    node: &dyn Node,
    name: &str,
    value: DataValue,
) -> Result<DataValue, EngineError> {
    let value = match node.definition().properties.iter().find(|p| p.name == name) {
        Some(p) => {
            normalize_property_value(&p.kind, &value).map_err(EngineError::InvalidProperty)?
        }
        None => value,
    };
    node.validate_property(name, &value)
        .map_err(EngineError::InvalidProperty)?;
    Ok(value)
}
