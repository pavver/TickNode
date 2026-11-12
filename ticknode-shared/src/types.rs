//! Named data types: core quantities (canonical numbers), plugin structs (tagged objects), and enums
//! (strings from an enumeration). The registry defines exact compatibility and explicit conversions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{DataType, DataValue, NumberValue, text};

mod conversion;
pub use conversion::Conversion;
use conversion::scalar_conversion;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldDefinition {
    pub name: String,
    pub title: String,
    pub description: String,
    pub data_type: DataType,
    /// An optional field may be absent from the value (Make does not require the input, Break may omit the output).
    pub optional: bool,
}

impl FieldDefinition {
    pub fn new(name: &str, title: &str, data_type: DataType) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: String::new(),
            data_type,
            optional: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn described(mut self, description: &str) -> Self {
        self.description = description.into();
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnumValue {
    /// The value as stored and passed (`DataValue::String`).
    pub value: String,
    /// Display label in the editor.
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TypeKind {
    Struct {
        fields: Vec<FieldDefinition>,
    },
    Enum {
        values: Vec<EnumValue>,
    },
    Quantity(crate::quantity::QuantityKind),
    /// Opaque signed integer; labels refer to an option source of the owning plugin.
    Identifier {
        labels: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypeDefinition {
    /// Full dotted name, e.g. `ha.state`.
    pub name: String,
    pub title: String,
    pub description: String,
    /// `#rrggbb` — color of pins and wires; if absent, the editor derives a color from the name hash.
    pub color: Option<String>,
    pub kind: TypeKind,
    /// Assigned by the host; never inferred from the dotted type name.
    #[serde(default)]
    pub plugin: Option<String>,
}

impl TypeDefinition {
    pub fn new_struct(name: &str, title: &str, fields: Vec<FieldDefinition>) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: String::new(),
            color: None,
            plugin: None,
            kind: TypeKind::Struct { fields },
        }
    }

    /// `values` — (value, label) pairs.
    pub fn new_enum(name: &str, title: &str, values: &[(&str, &str)]) -> Self {
        let values = values
            .iter()
            .map(|(v, t)| EnumValue {
                value: (*v).into(),
                title: (*t).into(),
            })
            .collect();
        Self {
            name: name.into(),
            title: title.into(),
            description: String::new(),
            color: None,
            plugin: None,
            kind: TypeKind::Enum { values },
        }
    }

    pub fn identifier(name: &str, title: &str, description: &str) -> Self {
        Self {
            name: name.into(),
            title: title.into(),
            description: description.into(),
            color: None,
            kind: TypeKind::Identifier { labels: None },
            plugin: None,
        }
    }

    pub fn labels(mut self, source: &str) -> Self {
        if let TypeKind::Identifier { labels } = &mut self.kind {
            *labels = Some(source.into());
        }
        self
    }

    pub fn is_identifier(&self) -> bool {
        matches!(self.kind, TypeKind::Identifier { .. })
    }

    pub fn described(mut self, description: &str) -> Self {
        self.description = description.into();
        self
    }

    pub fn colored(mut self, color: &str) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn fields(&self) -> &[FieldDefinition] {
        match &self.kind {
            TypeKind::Struct { fields } => fields,
            _ => &[],
        }
    }

    pub fn values(&self) -> &[EnumValue] {
        match &self.kind {
            TypeKind::Enum { values } => values,
            _ => &[],
        }
    }

    pub fn is_struct(&self) -> bool {
        matches!(self.kind, TypeKind::Struct { .. })
    }

    pub fn quantity(&self) -> Option<&crate::quantity::QuantityKind> {
        match &self.kind {
            TypeKind::Quantity(q) => Some(q),
            _ => None,
        }
    }

    pub fn is_enum(&self) -> bool {
        matches!(self.kind, TypeKind::Enum { .. })
    }
}

/// Whether this is a valid custom type name: dot-separated `[a-z0-9_-]` segments, with at least one dot
/// (which distinguishes custom type names from built-in types).
pub fn is_type_name(s: &str) -> bool {
    s.contains('.')
        && s.split('.').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// Registry of declared types (keyed by full name).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeRegistry {
    types: BTreeMap<String, TypeDefinition>,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_definitions(defs: impl IntoIterator<Item = TypeDefinition>) -> Self {
        Self {
            types: defs.into_iter().map(|d| (d.name.clone(), d)).collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&TypeDefinition> {
        self.types.get(name)
    }

    /// Type definition if `t` is `Named`.
    pub fn definition_of(&self, t: &DataType) -> Option<&TypeDefinition> {
        match t {
            DataType::Named(name) => self.get(name),
            _ => None,
        }
    }

    /// Removes a type (when a plugin is disabled). Handling any remaining references is up to the caller.
    pub fn remove(&mut self, name: &str) -> Option<TypeDefinition> {
        self.types.remove(name)
    }

    pub fn definitions(&self) -> Vec<TypeDefinition> {
        self.types.values().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Inserts a batch of types (from a single plugin) atomically. Fails on name conflict, invalid name,
    /// unknown field type, or an empty or duplicate enum.
    pub fn insert_all(&mut self, defs: &[TypeDefinition]) -> Result<(), String> {
        let mut next = self.clone();
        for def in defs {
            if def.name.starts_with("quantity.") || def.quantity().is_some() {
                return Err(format!(
                    "Data type {} uses the reserved quantity namespace or kind",
                    def.name
                ));
            }
            if !is_type_name(&def.name) {
                return Err(format!(
                    "Invalid type name {:?}: expected something like \"plugin.type\"",
                    def.name
                ));
            }
            if next.types.insert(def.name.clone(), def.clone()).is_some() {
                return Err(format!("Data type {} is already registered", def.name));
            }
        }
        for def in defs {
            next.validate_definition(def)?;
        }
        *self = next;
        Ok(())
    }

    fn validate_definition(&self, def: &TypeDefinition) -> Result<(), String> {
        match &def.kind {
            TypeKind::Struct { fields } => self.validate_fields(&def.name, fields),
            TypeKind::Enum { values } => validate_enum(&def.name, values),
            TypeKind::Quantity(_) => Ok(()),
            TypeKind::Identifier {
                labels: Some(source),
            } if source.is_empty() || source.len() > crate::MAX_OPTION_STRING_LEN => Err(format!(
                "Type {}: empty or oversized identifier label source",
                def.name
            )),
            TypeKind::Identifier { .. } => Ok(()),
        }
    }

    fn validate_fields(&self, name: &str, fields: &[FieldDefinition]) -> Result<(), String> {
        let mut seen = Vec::new();
        for field in fields {
            if field.name.is_empty() || seen.contains(&field.name) {
                return Err(format!(
                    "Type {name}: empty or duplicate field name {:?}",
                    field.name
                ));
            }
            seen.push(field.name.clone());
            self.check_known(&field.data_type)
                .map_err(|e| format!("Type {name}, field {}: {e}", field.name))?;
        }
        Ok(())
    }

    /// All `Named` types inside `t` must be registered.
    pub fn check_known(&self, t: &DataType) -> Result<(), String> {
        match t {
            DataType::Named(name) if !self.types.contains_key(name) => {
                Err(format!("unknown data type {name}"))
            }
            DataType::List(inner) => self.check_known(inner),
            _ => Ok(()),
        }
    }

    /// Human-readable type name: `title` for a custom type, otherwise the technical name.
    pub fn title_of(&self, t: &DataType) -> String {
        match t {
            DataType::Named(name) => self
                .get(name)
                .map_or_else(|| name.clone(), |d| d.title.clone()),
            DataType::List(inner) => format!("list<{}>", self.title_of(inner)),
            other => crate::data_type_name(other),
        }
    }

    /// Whether an output of type `src` can be connected to an input of type `dst` (see module docs for rules).
    pub fn compatible(&self, src: &DataType, dst: &DataType) -> bool {
        match (src, dst) {
            (DataType::Any, _) | (_, DataType::Any) => true,
            (DataType::List(a), DataType::List(b)) => self.compatible(a, b),
            (DataType::Named(a), DataType::Named(b)) => a == b,
            (DataType::Named(a), DataType::Number) => {
                self.get(a).is_some_and(|d| d.quantity().is_some())
            }
            (DataType::Named(a), DataType::Object) => {
                self.get(a).is_some_and(TypeDefinition::is_struct)
            }
            (DataType::Named(a), DataType::String) => {
                self.get(a).is_some_and(TypeDefinition::is_enum)
            }
            (DataType::Named(_), _) | (_, DataType::Named(_)) => false,
            (a, b) => a == b,
        }
    }

    /// Returns a conversion only for differing, convertible types. Compatible types need no conversion.
    /// Lists convert element by element; one invalid element makes the whole input unavailable.
    pub fn convertible(&self, src: &DataType, dst: &DataType) -> Option<Conversion> {
        use DataType::*;
        if self.compatible(src, dst) {
            return None;
        }
        Some(match (src, dst) {
            (Number, Named(name)) if self.get(name)?.quantity().is_some() => {
                Conversion::CanonicalNumber
            }
            (Named(name), String) if self.get(name)?.quantity().is_some() => {
                Conversion::FormatQuantity {
                    name: name.clone(),
                    kind: self.get(name)?.quantity()?.clone(),
                }
            }
            (String, Named(name)) => {
                let def = self.get(name)?;
                if let Some(q) = def.quantity() {
                    return Some(Conversion::ParseQuantity(q.clone()));
                }
                if !def.is_enum() {
                    return None;
                }
                Conversion::StringToEnum(def.values().iter().map(|v| v.value.clone()).collect())
            }
            (List(a), List(b)) => Conversion::List {
                element_type: (**b).clone(),
                conversion: Box::new(self.convertible(a, b)?),
            },
            _ => scalar_conversion(src, dst)?,
        })
    }

    /// Checks integer representations wherever a declared type contains identifiers.
    pub fn identifier_value_fits(&self, t: &DataType, value: &DataValue) -> bool {
        match (t, value) {
            (DataType::Named(name), value)
                if self.get(name).is_some_and(TypeDefinition::is_identifier) =>
            {
                matches!(value, DataValue::Number(NumberValue::Int(_)))
            }
            (DataType::List(inner), DataValue::List { elements, .. }) => elements
                .iter()
                .all(|v| self.identifier_value_fits(inner, v)),
            (DataType::Named(name), DataValue::Object { fields, .. }) => {
                self.get(name).is_none_or(|d| {
                    d.fields().iter().all(|f| {
                        fields
                            .get(&f.name)
                            .is_none_or(|v| self.identifier_value_fits(&f.data_type, &v.value))
                    })
                })
            }
            _ => true,
        }
    }

    /// Registry-aware initial value for editable named types.
    pub fn default_value(&self, t: &DataType) -> DataValue {
        if self
            .definition_of(t)
            .is_some_and(TypeDefinition::is_identifier)
        {
            return 0i64.into();
        }
        self.default_enum(t)
            .map(DataValue::String)
            .unwrap_or_else(|| text::default_of(t))
    }

    /// Default value for enum `t` — the first value in the enumeration.
    pub fn default_enum(&self, t: &DataType) -> Option<String> {
        self.definition_of(t)
            .and_then(|d| d.values().first())
            .map(|v| v.value.clone())
    }
}

fn validate_enum(name: &str, values: &[EnumValue]) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!("Type {name}: empty enumeration"));
    }
    let mut seen = Vec::new();
    for value in values {
        if seen.contains(&&value.value) {
            return Err(format!("Type {name}: duplicate value {:?}", value.value));
        }
        seen.push(&value.value);
    }
    Ok(())
}

#[cfg(test)]
mod identifier_tests;
#[cfg(test)]
mod tests;
