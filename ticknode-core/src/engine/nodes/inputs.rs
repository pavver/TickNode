use crate::{EngineError, TickEngine};
use ticknode_shared::*;

pub(super) enum PendingInput {
    Value(PinId, DataValue),
    Variable(PinId, VariableRef),
}

impl TickEngine {
    /// Value of an unconnected data input (`None` clears it). Effective while nothing is connected to the pin.
    pub fn set_input_value(
        &mut self,
        node_id: NodeId,
        pin: PinId,
        value: Option<DataValue>,
    ) -> Result<(), EngineError> {
        let node = self
            .nodes
            .get(&node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let definition = self
            .node_definition(node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        let values = self.input_values.entry(node_id).or_default();
        match value {
            Some(value) => {
                let pin_type =
                    input_pin_type(&definition, pin).ok_or(EngineError::InvalidPinConnection)?;
                if !input_value_fits(&self.types, &pin_type, &value) {
                    return Err(EngineError::TypeMismatch(value.get_type(), pin_type));
                }
                values.insert(pin.0, value);
                if let Some(vars) = self.input_variables.get_mut(&node_id) {
                    vars.remove(&pin.0);
                }
            }
            None => {
                values.remove(&pin.0);
            }
        }
        self.pure_cache.clear();
        self.clear_node_error(node_id);
        Ok(())
    }

    /// Binds an unconnected data input to a variable reference (`None` clears it).
    pub fn set_input_variable(
        &mut self,
        node_id: NodeId,
        pin: PinId,
        variable: Option<VariableRef>,
    ) -> Result<(), EngineError> {
        let node = self
            .nodes
            .get(&node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        if let Some(reason) = node.unavailable() {
            return Err(EngineError::PluginUnavailable(reason));
        }
        let definition = self
            .node_definition(node_id)
            .ok_or(EngineError::NodeNotFound(node_id))?;
        if input_pin_type(&definition, pin).is_none() {
            return Err(EngineError::InvalidPinConnection);
        }
        let vars = self.input_variables.entry(node_id).or_default();
        match variable {
            Some(var_ref) => {
                vars.insert(pin.0, var_ref);
                if let Some(values) = self.input_values.get_mut(&node_id) {
                    values.remove(&pin.0);
                }
            }
            None => {
                vars.remove(&pin.0);
            }
        }
        self.pure_cache.clear();
        self.clear_node_error(node_id);
        Ok(())
    }

    pub fn input_variable(&self, node_id: NodeId, pin: PinId) -> Option<&VariableRef> {
        self.input_variables
            .get(&node_id)
            .and_then(|vars| vars.get(&pin.0))
    }
}

/// Data type of an input pin (`None` if no such pin exists or it is an exec pin).
pub(crate) fn input_pin_type(definition: &NodeDefinition, pin: PinId) -> Option<DataType> {
    definition
        .pins
        .iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Input)
        .and_then(|p| match &p.pin_type {
            PinType::Data(t) => Some(t.clone()),
            PinType::Exec => None,
        })
}
