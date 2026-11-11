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

pub fn fit_typed_variable_value(types: &TypeRegistry, value: DataValue, source: &DataType, target: &DataType,
    rules: &crate::DisplayRules, labels: &dyn crate::quantity::QuantityLabels) -> Option<DataValue> {
    let opaque = |mut t: &DataType| {
        while let DataType::List(inner) = t { t = inner; }
        types.definition_of(t).is_some_and(|d| d.is_identifier())
    };
    if opaque(source) || opaque(target) {
        return (types.compatible(source, target) && types.identifier_value_fits(target, &value)).then_some(value);
    }
    fit_variable_value_with_display(types, value, target, rules, labels)
}

pub fn fit_variable_value(types: &TypeRegistry, value: DataValue, target_type: &DataType) -> Option<DataValue> {
    fit_variable_value_with_labels(types, value, target_type, &crate::quantity::CanonicalLabels)
}

pub fn fit_variable_value_with_labels(types: &TypeRegistry, value: DataValue, target_type: &DataType, labels: &dyn crate::quantity::QuantityLabels) -> Option<DataValue> {
    fit_variable_value_with_display(types, value, target_type, &Default::default(), labels)
}

pub fn fit_variable_value_with_display(types: &TypeRegistry, value: DataValue, target_type: &DataType,
    rules: &crate::DisplayRules, labels: &dyn crate::quantity::QuantityLabels) -> Option<DataValue> {
    if types.definition_of(target_type).is_some_and(|d| d.is_identifier()) {
        return matches!(value, DataValue::Number(crate::NumberValue::Int(_))).then_some(value);
    }
    if !types.identifier_value_fits(target_type, &value) { return None; }
    let src = value.get_type();
    if types.compatible(&src, target_type) {
        Some(value)
    } else if let Some(conv) = types.convertible(&src, target_type) {
        conv.apply_with_display(value, rules, labels)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
