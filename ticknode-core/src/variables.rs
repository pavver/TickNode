use std::collections::{BTreeMap, HashSet};
use ticknode_shared::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableError {
    Missing(String),
    TypeMismatch(String),
    LimitReached,
    InvalidName(String),
}

impl VariableError {
    pub fn ui_message(&self) -> UiMessage {
        match self {
            Self::Missing(name) => UiMessage::new(
                "error-variable-missing",
                format!("Variable \"{name}\" not found"),
            )
            .arg("name", name),
            Self::TypeMismatch(name) => UiMessage::new(
                "error-variable-type",
                format!("Value does not fit the type of variable \"{name}\""),
            )
            .arg("name", name),
            Self::LimitReached => UiMessage::new("error-variable-limit", "Variable limit reached"),
            Self::InvalidName(name) => UiMessage::new(
                "error-variable-name",
                format!("Invalid variable name \"{name}\""),
            )
            .arg("name", name),
        }
    }

    pub fn log_message(&self) -> String {
        format!("[ERROR] {}", self.ui_message().fallback)
    }
}

#[derive(Debug, Clone, Default)]
pub struct VariableStore {
    variables: BTreeMap<VariableKey, Variable>,
    pending_changes: BTreeMap<VariableKey, Option<Variable>>,
    persistent_dirty: bool,
}

impl VariableStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn clear(&mut self) {
        for key in self.variables.keys().cloned() {
            self.pending_changes.insert(key, None);
        }
        self.variables.clear();
        self.persistent_dirty = true;
    }

    pub fn contains_key(&self, key: &VariableKey) -> bool {
        self.variables.contains_key(key)
    }

    pub fn get(&self, key: &VariableKey) -> Option<&Variable> {
        self.variables.get(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&VariableKey, &Variable)> {
        self.variables.iter()
    }

    /// Resolves a variable name to its key according to the lookup scope.
    pub fn resolve(
        &self,
        name: &str,
        lookup: VariableLookup,
        sheet: Option<SheetId>,
    ) -> Option<VariableKey> {
        match lookup {
            VariableLookup::Auto => {
                if let Some(s) = sheet {
                    let k = VariableKey::sheet(s, name);
                    if self.variables.contains_key(&k) {
                        return Some(k);
                    }
                }
                let g = VariableKey::global(name);
                if self.variables.contains_key(&g) {
                    return Some(g);
                }
                let p = VariableKey::persistent(name);
                if self.variables.contains_key(&p) {
                    return Some(p);
                }
                None
            }
            VariableLookup::Sheet => {
                let s = sheet?;
                let k = VariableKey::sheet(s, name);
                self.variables.contains_key(&k).then_some(k)
            }
            VariableLookup::Global => {
                let g = VariableKey::global(name);
                if self.variables.contains_key(&g) {
                    return Some(g);
                }
                let p = VariableKey::persistent(name);
                if self.variables.contains_key(&p) {
                    return Some(p);
                }
                None
            }
        }
    }

    /// Checks if a variable with `name` exists in the target scope's namespace.
    pub fn exists_in_scope_namespace(
        &self,
        name: &str,
        scope: VariableScope,
        sheet: Option<SheetId>,
    ) -> bool {
        match scope {
            VariableScope::Sheet => {
                sheet.is_some_and(|s| self.variables.contains_key(&VariableKey::sheet(s, name)))
            }
            VariableScope::Global | VariableScope::Persistent => {
                self.variables.contains_key(&VariableKey::global(name))
                    || self.variables.contains_key(&VariableKey::persistent(name))
            }
        }
    }

    /// Creates a variable. If the name already exists in that scope's namespace, does nothing.
    /// Returns true if created, false if it already existed.
    pub fn create(&mut self, key: VariableKey, data_type: DataType, value: DataValue) -> bool {
        if self.exists_in_scope_namespace(&key.name, key.scope, key.sheet) {
            return false;
        }
        if key.scope == VariableScope::Persistent {
            self.persistent_dirty = true;
        }
        let var = Variable {
            key: key.clone(),
            data_type,
            value,
        };
        self.pending_changes.insert(key.clone(), Some(var.clone()));
        self.variables.insert(key, var);
        true
    }

    /// Sets the value of an existing variable.
    /// Returns true if value actually changed, false if value was identical or variable not found.
    pub fn set(&mut self, key: &VariableKey, value: DataValue) -> bool {
        if let Some(existing) = self.variables.get_mut(key) {
            if existing.value != value {
                existing.value = value;
                let var = existing.clone();
                self.pending_changes.insert(key.clone(), Some(var));
                if key.scope == VariableScope::Persistent {
                    self.persistent_dirty = true;
                }
                return true;
            }
        }
        false
    }

    /// Deletes a variable by key. Returns true if removed, false if not found.
    pub fn delete(&mut self, key: &VariableKey) -> bool {
        if self.variables.remove(key).is_some() {
            self.pending_changes.insert(key.clone(), None);
            if key.scope == VariableScope::Persistent {
                self.persistent_dirty = true;
            }
            return true;
        }
        false
    }

    /// Drops all sheet variables for the given sheet.
    pub fn remove_sheet(&mut self, sheet: SheetId) {
        let to_remove: Vec<VariableKey> = self
            .variables
            .keys()
            .filter(|k| k.scope == VariableScope::Sheet && k.sheet == Some(sheet))
            .cloned()
            .collect();
        for key in to_remove {
            self.variables.remove(&key);
            self.pending_changes.insert(key, None);
        }
    }

    /// Drains coalesced changes since the previous call.
    pub fn take_changes(&mut self) -> VariableChanges {
        let mut set = Vec::new();
        let mut removed = Vec::new();
        for (key, opt) in std::mem::take(&mut self.pending_changes) {
            match opt {
                Some(var) => set.push(var),
                None => removed.push(key),
            }
        }
        VariableChanges { set, removed }
    }

    pub fn is_persistent_dirty(&self) -> bool {
        self.persistent_dirty
    }

    pub fn clear_persistent_dirty(&mut self) {
        self.persistent_dirty = false;
    }

    pub fn persistent_variables(&self) -> Vec<Variable> {
        let mut vars: Vec<_> = self
            .variables
            .values()
            .filter(|v| v.key.scope == VariableScope::Persistent)
            .cloned()
            .collect();
        vars.sort_by(|a, b| a.key.name.cmp(&b.key.name));
        vars
    }

    pub fn for_sheet_and_global(&self, sheet: Option<SheetId>) -> Vec<Variable> {
        self.variables
            .values()
            .filter(|v| {
                v.key.scope == VariableScope::Global
                    || v.key.scope == VariableScope::Persistent
                    || (v.key.scope == VariableScope::Sheet
                        && sheet.is_some()
                        && v.key.sheet == sheet)
            })
            .cloned()
            .collect()
    }

    pub fn active_sheet_variables(&self) -> HashSet<(SheetId, String)> {
        self.variables
            .keys()
            .filter_map(|k| {
                if k.scope == VariableScope::Sheet
                    && let Some(s) = k.sheet
                {
                    Some((s, k.name.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
