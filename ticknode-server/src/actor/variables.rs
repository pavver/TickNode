//! Variable commands from the editor: validation and changes in the engine store.

use super::*;
use ticknode_shared::{
    fit_variable_value_with_display, valid_variable_name, valid_variable_value, DataValue, Variable, VariableKey, VariableScope, MAX_VARIABLES,
};

impl Actor {
    pub(super) fn create_variable(&mut self, var: Variable) -> Result<(), UiMessage> {
        if !valid_variable_name(&var.key.name) {
            return Err(UiMessage::new("error-variable-name", "Invalid variable name").arg("name", &var.key.name));
        }
        match var.key.scope {
            VariableScope::Sheet => {
                let Some(sheet_id) = var.key.sheet else {
                    return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                };
                if !self.sheets.iter().any(|s| s.id == sheet_id) {
                    return Err(UiMessage::new("error-sheet-not-found", "Sheet not found"));
                }
            }
            VariableScope::Global | VariableScope::Persistent => {
                if var.key.sheet.is_some() {
                    return Err(UiMessage::new("error-variable-scope", "Global and persistent variables cannot have a sheet"));
                }
            }
        }
        if self.engine.variables.len() >= MAX_VARIABLES && !self.engine.variables.contains_key(&var.key) {
            return Err(UiMessage::new("error-variable-limit", "Variable limit reached (maximum 10,000)"));
        }
        if !valid_variable_value(&var.value) {
            return Err(UiMessage::new("error-variable-limit", "Variable value exceeds maximum size (64 KiB)"));
        }
        let Some(fitted_value) = fit_variable_value_with_display(&self.engine.types, var.value, &var.data_type, &self.engine.host_state.display, ticknode_core::quantity::labels(&self.engine.host_state.locale)) else {
            return Err(UiMessage::new("error-variable-type", "Value does not match variable type"));
        };
        if !self.engine.variables.create(var.key, var.data_type, fitted_value) {
            return Err(UiMessage::new("error-variable-exists", "Variable already exists in this scope"));
        }
        if self.engine.variables.is_persistent_dirty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok(())
    }

    pub(super) fn set_variable(&mut self, key: VariableKey, value: DataValue) -> Result<(), UiMessage> {
        if !valid_variable_value(&value) {
            return Err(UiMessage::new("error-variable-limit", "Variable value exceeds maximum size (64 KiB)"));
        }
        let Some(existing) = self.engine.variables.get(&key) else {
            return Err(UiMessage::new("error-variable-missing", "Variable not found").arg("name", &key.name));
        };
        let Some(fitted) = fit_variable_value_with_display(&self.engine.types, value, &existing.data_type, &self.engine.host_state.display, ticknode_core::quantity::labels(&self.engine.host_state.locale)) else {
            return Err(UiMessage::new("error-variable-type", "Value does not match variable type"));
        };
        self.engine.variables.set(&key, fitted);
        if self.engine.variables.is_persistent_dirty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok(())
    }

    pub(super) fn delete_variable(&mut self, key: VariableKey) -> Result<(), UiMessage> {
        self.engine.variables.delete(&key);
        if self.engine.variables.is_persistent_dirty() {
            self.dirty_since.get_or_insert_with(Instant::now);
        }
        Ok(())
    }
}
