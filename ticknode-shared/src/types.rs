//! Custom data types declared by plugins: structs (`DataValue::Object` with a type tag)
//! and enums (`DataValue::String` with a value from the enumeration), plus their registry.
//!
//! Typing is strict: `Named(a)` is compatible only with `Named(a)`, `Any`, and `Object` (for structs)
//! or `String` (for enums). Not the other way around: `Object`/`String` cannot be fed into a custom-type
//! pin without conversion. Explicit conversion rules are separate from compatibility.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{DataType, DataValue, NumberValue, text};

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
        Self { name: name.into(), title: title.into(), description: String::new(), data_type, optional: false }
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypeKind {
    Struct { fields: Vec<FieldDefinition> },
    Enum { values: Vec<EnumValue> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeDefinition {
    /// Full dotted name, e.g. `ha.state`.
    pub name: String,
    pub title: String,
    pub description: String,
    /// `#rrggbb` — color of pins and wires; if absent, the editor derives a color from the name hash.
    pub color: Option<String>,
    pub kind: TypeKind,
}

impl TypeDefinition {
    pub fn new_struct(name: &str, title: &str, fields: Vec<FieldDefinition>) -> Self {
        Self { name: name.into(), title: title.into(), description: String::new(), color: None, kind: TypeKind::Struct { fields } }
    }

    /// `values` — (value, label) pairs.
    pub fn new_enum(name: &str, title: &str, values: &[(&str, &str)]) -> Self {
        let values = values.iter().map(|(v, t)| EnumValue { value: (*v).into(), title: (*t).into() }).collect();
        Self { name: name.into(), title: title.into(), description: String::new(), color: None, kind: TypeKind::Enum { values } }
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
            TypeKind::Enum { .. } => &[],
        }
    }

    pub fn values(&self) -> &[EnumValue] {
        match &self.kind {
            TypeKind::Enum { values } => values,
            TypeKind::Struct { .. } => &[],
        }
    }

    pub fn is_struct(&self) -> bool {
        matches!(self.kind, TypeKind::Struct { .. })
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
            !seg.is_empty() && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// A conversion applied while pulling a connected input. Failure means no input value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversion {
    Parse(DataType),
    Format(DataType),
    StringToBool,
    NumberToBool,
    BoolToNumber,
    StringToEnum(Vec<String>),
    List { element_type: DataType, conversion: Box<Conversion> },
}

impl Conversion {
    pub fn apply(&self, value: DataValue) -> Option<DataValue> {
        match (self, value) {
            (Self::Parse(dst), DataValue::String(s)) => text::parse_value(&s, dst).ok(),
            (Self::Format(src), value) if value.get_type() == *src => Some(DataValue::String(text::format_value(&value))),
            (Self::StringToBool, DataValue::String(s)) => {
                parse_switch_state(&s).map(DataValue::Bool)
            }
            (Self::NumberToBool, DataValue::Number(n)) if n.to_f64().is_finite() => Some(DataValue::Bool(n.to_f64() != 0.0)),
            (Self::BoolToNumber, DataValue::Bool(b)) => Some(DataValue::Number(NumberValue::Int(i64::from(b)))),
            (Self::StringToEnum(values), DataValue::String(s)) => values.contains(&s).then_some(DataValue::String(s)),
            (Self::List { element_type, conversion }, DataValue::List { elements, .. }) => convert_list(element_type, conversion, elements),
            _ => None,
        }
    }
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
        Self { types: defs.into_iter().map(|d| (d.name.clone(), d)).collect() }
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
            if !is_type_name(&def.name) {
                return Err(format!("Invalid type name {:?}: expected something like \"plugin.type\"", def.name));
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
        }
    }

    fn validate_fields(&self, name: &str, fields: &[FieldDefinition]) -> Result<(), String> {
        let mut seen = Vec::new();
        for field in fields {
            if field.name.is_empty() || seen.contains(&field.name) {
                return Err(format!("Type {name}: empty or duplicate field name {:?}", field.name));
            }
            seen.push(field.name.clone());
            self.check_known(&field.data_type).map_err(|e| format!("Type {name}, field {}: {e}", field.name))?;
        }
        Ok(())
    }

    /// All `Named` types inside `t` must be registered.
    pub fn check_known(&self, t: &DataType) -> Result<(), String> {
        match t {
            DataType::Named(name) if !self.types.contains_key(name) => Err(format!("unknown data type {name}")),
            DataType::List(inner) => self.check_known(inner),
            _ => Ok(()),
        }
    }

    /// Human-readable type name: `title` for a custom type, otherwise the technical name.
    pub fn title_of(&self, t: &DataType) -> String {
        match t {
            DataType::Named(name) => self.get(name).map_or_else(|| name.clone(), |d| d.title.clone()),
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
            (DataType::Named(a), DataType::Object) => self.get(a).is_some_and(TypeDefinition::is_struct),
            (DataType::Named(a), DataType::String) => self.get(a).is_some_and(TypeDefinition::is_enum),
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
            (String, Named(name)) => {
                let def = self.get(name).filter(|d| d.is_enum())?;
                Conversion::StringToEnum(def.values().iter().map(|v| v.value.clone()).collect())
            }
            (List(a), List(b)) => Conversion::List { element_type: (**b).clone(), conversion: Box::new(self.convertible(a, b)?) },
            _ => scalar_conversion(src, dst)?,
        })
    }

    /// Default value for enum `t` — the first value in the enumeration.
    pub fn default_enum(&self, t: &DataType) -> Option<String> {
        self.definition_of(t).and_then(|d| d.values().first()).map(|v| v.value.clone())
    }
}

fn convert_list(element_type: &DataType, conversion: &Conversion, elements: Vec<DataValue>) -> Option<DataValue> {
    Some(DataValue::List {
        element_type: element_type.clone(),
        elements: elements.into_iter().map(|value| conversion.apply(value)).collect::<Option<Vec<_>>>()?,
    })
}

fn scalar_conversion(src: &DataType, dst: &DataType) -> Option<Conversion> {
    use DataType::*;
    Some(match (src, dst) {
        (String, Number | Time | DateTime | Duration | Color) => Conversion::Parse(dst.clone()),
        (Number | Bool | Time | DateTime | Duration | Color, String) => Conversion::Format(src.clone()),
        (String, Bool) => Conversion::StringToBool,
        (Number, Bool) => Conversion::NumberToBool,
        (Bool, Number) => Conversion::BoolToNumber,
        _ => return None,
    })
}

fn parse_switch_state(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "open" | "home" | "playing" | "locked" | "detected" | "active" | "occupied" | "connected" => Some(true),
        "off" | "false" | "no" | "0" | "closed" | "not_home" | "paused" | "unlocked" | "clear" | "inactive" | "unoccupied" | "disconnected" => Some(false),
        _ => None,
    }
}

fn validate_enum(name: &str, values: &[EnumValue]) -> Result<(), String> {
    if values.is_empty() { return Err(format!("Type {name}: empty enumeration")); }
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
mod tests {
    use super::*;
    use crate::condition::{CompareOp, MatchCase, MatchInput, ops_in};
    use crate::{DataValue, NumberValue, input_value_editable, input_value_fits, parse_data_type, text};

    fn registry() -> TypeRegistry {
        let mut r = TypeRegistry::new();
        r.insert_all(&[
            TypeDefinition::new_struct("t.point", "Point", vec![
                FieldDefinition::new("x", "X", DataType::Number),
                FieldDefinition::new("label", "Label", DataType::String).optional(),
            ]),
            TypeDefinition::new_enum("t.mode", "Mode", &[("auto", "Auto"), ("manual", "Manual")]),
        ]).unwrap();
        r
    }

    fn named(n: &str) -> DataType {
        DataType::Named(n.into())
    }

    #[test]
    fn primitive_conversions_and_failures() {
        use DataType::*;
        let r = registry();
        let cases = [
            (String, Number, DataValue::from("21.5"), Some(DataValue::from(21.5))),
            (String, Number, DataValue::from("unavailable"), None),
            (String, Number, DataValue::from("NaN"), None),
            (String, Number, DataValue::from("inf"), None),
            (Number, String, DataValue::from(42i64), Some(DataValue::from("42"))),
            (Number, String, DataValue::from(21.5), Some(DataValue::from("21.5"))),
            (Bool, String, DataValue::Bool(false), Some(DataValue::from("false"))),
            (Number, Bool, DataValue::from(-2i64), Some(DataValue::Bool(true))),
            (Number, Bool, DataValue::from(0i64), Some(DataValue::Bool(false))),
            (Bool, Number, DataValue::Bool(true), Some(DataValue::from(1i64))),
            (Bool, Number, DataValue::Bool(false), Some(DataValue::from(0i64))),
            (String, named("t.mode"), DataValue::from("auto"), Some(DataValue::from("auto"))),
            (String, named("t.mode"), DataValue::from("Auto"), None),
            (String, named("t.mode"), DataValue::from("other"), None),
        ];
        for (src, dst, value, expected) in cases {
            assert_eq!(r.convertible(&src, &dst).unwrap().apply(value.clone()), expected, "{src:?} -> {dst:?}: {value:?}");
        }
        for (yes, no) in [("on", "off"), ("true", "false"), ("yes", "no"), ("1", "0"), ("open", "closed"), ("home", "not_home"), ("playing", "paused"), ("locked", "unlocked"), ("detected", "clear"), ("active", "inactive"), ("occupied", "unoccupied"), ("connected", "disconnected"), (" ON ", " OFF ")] {
            let conv = r.convertible(&String, &Bool).unwrap();
            assert_eq!(conv.apply(DataValue::from(yes)), Some(DataValue::Bool(true)));
            assert_eq!(conv.apply(DataValue::from(no)), Some(DataValue::Bool(false)));
        }
        for unknown in ["unknown", "unavailable", "", "2"] {
            assert_eq!(r.convertible(&String, &Bool).unwrap().apply(DataValue::from(unknown)), None);
        }
        for (dst, input) in [(Time, "07:30"), (DateTime, "2026-10-31 12:45"), (Duration, "1h 30m"), (Color, "#12345678")] {
            let value = text::parse_value(input, &dst).unwrap();
            assert_eq!(r.convertible(&String, &dst).unwrap().apply(DataValue::from(input)), Some(value.clone()));
            assert_eq!(r.convertible(&dst, &String).unwrap().apply(value.clone()), Some(DataValue::from(text::format_value(&value))));
            assert_eq!(r.convertible(&String, &dst).unwrap().apply(DataValue::from("invalid")), None);
        }
        for (src, dst) in [(Number, Number), (Any, Number), (named("t.mode"), String), (Object, named("t.point")), (String, named("t.point")), (Number, Time), (named("t.mode"), Number)] {
            assert_eq!(r.convertible(&src, &dst), None, "{src:?} -> {dst:?}");
        }
    }

    #[test]
    fn list_conversion_is_elementwise_and_atomic() {
        let r = registry();
        let list = |t| DataType::List(Box::new(t));
        let conv = r.convertible(&list(DataType::String), &list(DataType::Number)).unwrap();
        let value = |texts: &[&str]| DataValue::List { element_type: DataType::String, elements: texts.iter().map(|s| DataValue::from(*s)).collect() };
        assert_eq!(conv.apply(value(&["1", "2.5"])), Some(DataValue::List { element_type: DataType::Number, elements: vec![DataValue::from(1i64), DataValue::from(2.5)] }));
        assert_eq!(conv.apply(value(&["1", "bad"])), None);
        assert_eq!(conv.apply(value(&[])), Some(DataValue::List { element_type: DataType::Number, elements: vec![] }));
        assert!(r.convertible(&list(DataType::Object), &list(DataType::Number)).is_none());
    }

    #[test]
    fn compatibility_rules() {
        let r = registry();
        assert!(r.compatible(&named("t.point"), &named("t.point")));
        assert!(!r.compatible(&named("t.point"), &named("t.mode")));
        assert!(r.compatible(&named("t.point"), &DataType::Any));
        assert!(r.compatible(&DataType::Any, &named("t.mode")));
        assert!(r.compatible(&named("t.point"), &DataType::Object));
        assert!(!r.compatible(&DataType::Object, &named("t.point")));
        assert!(r.compatible(&named("t.mode"), &DataType::String));
        assert!(!r.compatible(&DataType::String, &named("t.mode")));
        assert!(!r.compatible(&named("t.mode"), &DataType::Object));
        assert!(!r.compatible(&named("t.point"), &DataType::String));
        assert!(r.compatible(&DataType::List(Box::new(named("t.point"))), &DataType::List(Box::new(DataType::Object))));
        assert!(!r.compatible(&named("unknown.type"), &DataType::Object));
    }

    #[test]
    fn registration_is_validated_atomically() {
        let mut r = registry();
        assert!(r.insert_all(&[TypeDefinition::new_enum("t.mode", "Dup", &[("a", "A")])]).is_err());
        let bad = TypeDefinition::new_struct("t.bad", "Bad", vec![FieldDefinition::new("p", "P", named("no.such"))]);
        let ok = TypeDefinition::new_enum("t.fine", "Fine", &[("a", "A")]);
        assert!(r.insert_all(&[ok, bad]).is_err());
        assert!(r.get("t.fine").is_none(), "a batch registers entirely or not at all");
        assert!(r.insert_all(&[TypeDefinition::new_enum("t.empty", "E", &[])]).is_err());
        assert!(r.insert_all(&[TypeDefinition::new_enum("NoDot", "E", &[("a", "A")])]).is_err());
        // A type may reference another type from the same batch
        let a = TypeDefinition::new_struct("t.a", "A", vec![FieldDefinition::new("b", "B", named("t.b"))]);
        let b = TypeDefinition::new_enum("t.b", "B", &[("x", "X")]);
        r.insert_all(&[a, b]).unwrap();
    }

    #[test]
    fn type_names_in_text() {
        assert_eq!(parse_data_type("t.point"), Some(named("t.point")));
        assert_eq!(parse_data_type("list<t.point>"), Some(DataType::List(Box::new(named("t.point")))));
        assert_eq!(parse_data_type("point"), None);
        assert_eq!(crate::data_type_name(&DataType::List(Box::new(named("t.point")))), "list<t.point>");
    }

    #[test]
    fn enum_text_and_input_values() {
        let r = registry();
        let mode = named("t.mode");
        assert_eq!(text::parse_value_in(&r, "auto", &mode), Ok(DataValue::String("auto".into())));
        assert_eq!(text::parse_value_in(&r, "Manual", &mode), Ok(DataValue::String("manual".into())));
        assert!(text::parse_value_in(&r, "other", &mode).is_err());
        assert!(text::parse_value_in(&r, "1", &named("t.point")).is_err());
        assert!(input_value_editable(&r, &mode));
        assert!(!input_value_editable(&r, &named("t.point")));
        assert!(input_value_fits(&r, &mode, &DataValue::String("manual".into())));
        assert!(!input_value_fits(&r, &mode, &DataValue::String("nope".into())));
        assert!(!input_value_fits(&r, &mode, &DataValue::Number(NumberValue::Int(1))));
    }

    #[test]
    fn tagged_object_has_named_type_and_survives_postcard() {
        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), crate::ObjectField { name: "x".into(), description: String::new(), value: DataValue::Number(NumberValue::Int(1)) });
        let v = DataValue::Object { fields, type_name: Some("t.point".into()) };
        assert_eq!(v.get_type(), named("t.point"));
        let back: DataValue = postcard::from_bytes(&postcard::to_allocvec(&v).unwrap()).unwrap();
        assert_eq!(back, v);
        // Legacy graph JSON (without type_name) deserializes as a plain object
        let old: DataValue = serde_json::from_str(r#"{"Object":{"fields":{}}}"#).unwrap();
        assert_eq!(old.get_type(), DataType::Object);
    }

    #[test]
    fn match_treats_struct_as_object_and_enum_as_string() {
        let r = registry();
        assert!(ops_in(&r, &named("t.point")).contains(&CompareOp::HasField));
        assert!(!ops_in(&r, &named("t.mode")).contains(&CompareOp::HasField));
        assert!(ops_in(&r, &named("t.mode")).contains(&CompareOp::Eq));
        let input = MatchInput::parse("1:p:t.point").unwrap();
        assert_eq!(input.value_type, named("t.point"));
        assert_eq!(input.to_text(), "1:p:t.point");

        let case = MatchCase::parse("1: p has_field \"label\" and m = \"auto\"").unwrap();
        let point = DataValue::Object {
            fields: [("label".to_string(), crate::ObjectField { name: "label".into(), description: String::new(), value: DataValue::String("a".into()) })].into(),
            type_name: Some("t.point".into()),
        };
        let value_of = |name: &str| match name {
            "p" => Some(point.clone()),
            "m" => Some(DataValue::String("auto".into())),
            _ => None,
        };
        assert!(case.matches(value_of));
    }
}
