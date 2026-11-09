use serde::{Deserialize, Serialize};
use crate::{DataType, DataValue, SheetId, TypeRegistry};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VariableScope {
    Sheet,
    Global,
    Persistent,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VariableKey {
    pub scope: VariableScope,
    pub sheet: Option<SheetId>,
    pub name: String,
}

impl VariableKey {
    pub fn sheet(sheet: SheetId, name: impl Into<String>) -> Self {
        Self {
            scope: VariableScope::Sheet,
            sheet: Some(sheet),
            name: name.into(),
        }
    }

    pub fn global(name: impl Into<String>) -> Self {
        Self {
            scope: VariableScope::Global,
            sheet: None,
            name: name.into(),
        }
    }

    pub fn persistent(name: impl Into<String>) -> Self {
        Self {
            scope: VariableScope::Persistent,
            sheet: None,
            name: name.into(),
        }
    }

    pub fn is_valid(&self) -> bool {
        valid_variable_name(&self.name) && (self.sheet.is_some() == (self.scope == VariableScope::Sheet))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub key: VariableKey,
    pub data_type: DataType,
    pub value: DataValue,
}

/// How a node finds a variable by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VariableLookup {
    /// Sheet first, then global or persistent.
    Auto,
    /// Sheet scope only.
    Sheet,
    /// Global or persistent scope.
    Global,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VariableChanges {
    pub set: Vec<Variable>,
    pub removed: Vec<VariableKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VariableCommand {
    Create {
        name: String,
        scope: VariableScope,
        data_type: DataType,
        value: DataValue,
    },
    Set {
        name: String,
        lookup: VariableLookup,
        value: DataValue,
    },
    Delete {
        name: String,
        lookup: VariableLookup,
    },
}

pub const MAX_VARIABLES: usize = 10_000;
pub const MAX_VARIABLE_VALUE_SIZE: usize = 64 * 1024; // 64 KiB

pub fn valid_variable_name(name: &str) -> bool {
    let len = name.chars().count();
    (1..=100).contains(&len) && name.trim() == name && !name.chars().any(|c| c.is_control())
}

pub fn valid_variable_value(value: &DataValue) -> bool {
    postcard::to_allocvec(value)
        .map(|bytes| bytes.len() <= MAX_VARIABLE_VALUE_SIZE)
        .unwrap_or(false)
}

pub fn fit_variable_value(types: &TypeRegistry, value: DataValue, target_type: &DataType) -> Option<DataValue> {
    let src = value.get_type();
    if types.compatible(&src, target_type) {
        Some(value)
    } else if let Some(conv) = types.convertible(&src, target_type) {
        conv.apply(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
