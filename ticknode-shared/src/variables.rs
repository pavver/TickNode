use crate::{DataType, DataValue, SheetId, TypeRegistry};
use serde::{Deserialize, Serialize};

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

    pub fn prefix(&self) -> &'static str {
        match self.scope {
            VariableScope::Sheet => "sheet",
            VariableScope::Global | VariableScope::Persistent => "global",
        }
    }

    pub fn is_valid(&self) -> bool {
        valid_variable_name(&self.name)
            && (self.sheet.is_some() == (self.scope == VariableScope::Sheet))
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
    /// Read-only plugin path stored in full in VariableRef::name.
    Plugin,
}

/// A reference to a variable using scoped syntax:
/// `var.<name>` (Auto), `sheet.<name>`, `global.<name>`, or a full read-only plugin path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VariableRef {
    pub lookup: VariableLookup,
    pub name: String,
}

impl VariableRef {
    pub fn new(lookup: VariableLookup, name: impl Into<String>) -> Self {
        Self {
            lookup,
            name: name.into(),
        }
    }

    pub fn auto(name: impl Into<String>) -> Self {
        Self::new(VariableLookup::Auto, name)
    }

    pub fn sheet(name: impl Into<String>) -> Self {
        Self::new(VariableLookup::Sheet, name)
    }

    pub fn global(name: impl Into<String>) -> Self {
        Self::new(VariableLookup::Global, name)
    }

    /// Parses scoped variable references or dotted paths under non-reserved plugin roots.
    pub fn parse(s: &str) -> Option<Self> {
        let (prefix, name) = s.split_once('.')?;
        let lookup = match prefix {
            "var" => VariableLookup::Auto,
            "sheet" => VariableLookup::Sheet,
            "global" => VariableLookup::Global,
            _ if crate::plugin_values::valid_path(s)
                && !crate::plugin_values::reserved_root(prefix) =>
            {
                return Some(Self::new(VariableLookup::Plugin, s));
            }
            _ => return None,
        };
        if valid_variable_name(name) {
            Some(Self {
                lookup,
                name: name.to_string(),
            })
        } else {
            None
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self.lookup {
            VariableLookup::Auto => "var",
            VariableLookup::Sheet => "sheet",
            VariableLookup::Global => "global",
            VariableLookup::Plugin => "",
        }
    }

    pub fn to_str(&self) -> String {
        if self.lookup == VariableLookup::Plugin {
            self.name.clone()
        } else {
            format!("{}.{}", self.prefix(), self.name)
        }
    }
}

impl std::fmt::Display for VariableRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
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

pub fn fit_typed_variable_value(
    types: &TypeRegistry,
    value: DataValue,
    source: &DataType,
    target: &DataType,
    rules: &crate::DisplayRules,
    labels: &dyn crate::quantity::QuantityLabels,
) -> Option<DataValue> {
    let mut value = value;
    crate::retain_field_types(&mut value, source, types);
    let opaque = |mut t: &DataType| {
        while let DataType::List(inner) = t {
            t = inner;
        }
        types.definition_of(t).is_some_and(|d| d.is_identifier())
    };
    if opaque(source) || opaque(target) {
        return (types.compatible(source, target) && types.identifier_value_fits(target, &value))
            .then_some(value);
    }
    fit_variable_value_with_display(types, value, target, rules, labels)
}

pub fn fit_variable_value(
    types: &TypeRegistry,
    value: DataValue,
    target_type: &DataType,
) -> Option<DataValue> {
    fit_variable_value_with_labels(types, value, target_type, &crate::quantity::CanonicalLabels)
}

pub fn fit_variable_value_with_labels(
    types: &TypeRegistry,
    value: DataValue,
    target_type: &DataType,
    labels: &dyn crate::quantity::QuantityLabels,
) -> Option<DataValue> {
    fit_variable_value_with_display(types, value, target_type, &Default::default(), labels)
}

pub fn fit_variable_value_with_display(
    types: &TypeRegistry,
    value: DataValue,
    target_type: &DataType,
    rules: &crate::DisplayRules,
    labels: &dyn crate::quantity::QuantityLabels,
) -> Option<DataValue> {
    if types
        .definition_of(target_type)
        .is_some_and(|d| d.is_identifier())
    {
        return matches!(value, DataValue::Number(crate::NumberValue::Int(_))).then_some(value);
    }
    if !types.identifier_value_fits(target_type, &value) {
        return None;
    }
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
