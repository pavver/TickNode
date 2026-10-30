//! Користувацькі типи даних, які оголошують плагіни: структури (`DataValue::Object` з тегом типу)
//! і переліки (`DataValue::String` зі значенням з переліку), та реєстр цих типів.
//!
//! Типізація строга: `Named(a)` сумісний лише з `Named(a)`, `Any` і (для структури) `Object`,
//! (для переліку) `String`. У зворотний бік — ні: `Object`/`String` не можна подати на пін
//! власного типу, бо вони не гарантують ні полів, ні допустимого значення.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::DataType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldDefinition {
    pub name: String,
    pub title: String,
    pub description: String,
    pub data_type: DataType,
    /// Необов'язкове поле може бути відсутнє у значенні (Make не вимагає входу, Break не видає значення).
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
    /// Значення, як воно зберігається й передається (`DataValue::String`).
    pub value: String,
    /// Як показувати в редакторі.
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypeKind {
    Struct { fields: Vec<FieldDefinition> },
    Enum { values: Vec<EnumValue> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeDefinition {
    /// Повна назва з крапкою, напр. `ha.state`.
    pub name: String,
    pub title: String,
    pub description: String,
    /// `#rrggbb` — колір пінів і дротів; без нього редактор бере колір із хешу назви.
    pub color: Option<String>,
    pub kind: TypeKind,
}

impl TypeDefinition {
    pub fn new_struct(name: &str, title: &str, fields: Vec<FieldDefinition>) -> Self {
        Self { name: name.into(), title: title.into(), description: String::new(), color: None, kind: TypeKind::Struct { fields } }
    }

    /// `values` — пари (значення, підпис).
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

/// Чи це припустима назва власного типу: сегменти `[a-z0-9_-]` через крапку, крапка обов'язкова
/// (так назву відрізняють від вбудованих типів).
pub fn is_type_name(s: &str) -> bool {
    s.contains('.')
        && s.split('.').all(|seg| {
            !seg.is_empty() && seg.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// Реєстр оголошених типів (за повною назвою).
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

    /// Визначення типу, якщо `t` — `Named`.
    pub fn definition_of(&self, t: &DataType) -> Option<&TypeDefinition> {
        match t {
            DataType::Named(name) => self.get(name),
            _ => None,
        }
    }

    /// Прибирає тип (плагін вимкнено). Чи лишились посилання на нього — справа викликача.
    pub fn remove(&mut self, name: &str) -> Option<TypeDefinition> {
        self.types.remove(name)
    }

    pub fn definitions(&self) -> Vec<TypeDefinition> {
        self.types.values().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Додає типи пачкою (один плагін): атомарно. Помилка — конфлікт назви, некоректна назва,
    /// невідомий тип у полі, порожній чи дубльований перелік.
    pub fn insert_all(&mut self, defs: &[TypeDefinition]) -> Result<(), String> {
        let mut next = self.clone();
        for def in defs {
            if !is_type_name(&def.name) {
                return Err(format!("Некоректна назва типу {:?}: очікується щось на кшталт «plugin.type»", def.name));
            }
            if next.types.insert(def.name.clone(), def.clone()).is_some() {
                return Err(format!("Тип даних {} уже зареєстровано", def.name));
            }
        }
        for def in defs {
            match &def.kind {
                TypeKind::Struct { fields } => {
                    let mut seen = Vec::new();
                    for f in fields {
                        if f.name.is_empty() || seen.contains(&f.name) {
                            return Err(format!("Тип {}: порожня або повторювана назва поля {:?}", def.name, f.name));
                        }
                        seen.push(f.name.clone());
                        next.check_known(&f.data_type).map_err(|e| format!("Тип {}, поле {}: {e}", def.name, f.name))?;
                    }
                }
                TypeKind::Enum { values } => {
                    if values.is_empty() {
                        return Err(format!("Тип {}: порожній перелік", def.name));
                    }
                    let mut seen = Vec::new();
                    for v in values {
                        if seen.contains(&&v.value) {
                            return Err(format!("Тип {}: значення {:?} повторюється", def.name, v.value));
                        }
                        seen.push(&v.value);
                    }
                }
            }
        }
        *self = next;
        Ok(())
    }

    /// Усі `Named` усередині `t` мають бути зареєстровані.
    pub fn check_known(&self, t: &DataType) -> Result<(), String> {
        match t {
            DataType::Named(name) if !self.types.contains_key(name) => Err(format!("невідомий тип даних {name}")),
            DataType::List(inner) => self.check_known(inner),
            _ => Ok(()),
        }
    }

    /// Назва типу для людини: `title` власного типу, інакше технічна назва.
    pub fn title_of(&self, t: &DataType) -> String {
        match t {
            DataType::Named(name) => self.get(name).map_or_else(|| name.clone(), |d| d.title.clone()),
            DataType::List(inner) => format!("list<{}>", self.title_of(inner)),
            other => crate::data_type_name(other),
        }
    }

    /// Чи можна з'єднати вихід типу `src` зі входом типу `dst` (див. правила в документації модуля).
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

    /// Значення переліку `t` за замовчуванням — перше з переліку.
    pub fn default_enum(&self, t: &DataType) -> Option<String> {
        self.definition_of(t).and_then(|d| d.values().first()).map(|v| v.value.clone())
    }
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
            TypeDefinition::new_enum("t.mode", "Mode", &[("auto", "Авто"), ("manual", "Вручну")]),
        ]).unwrap();
        r
    }

    fn named(n: &str) -> DataType {
        DataType::Named(n.into())
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
        assert!(r.get("t.fine").is_none(), "пачка або проходить цілком, або ні");
        assert!(r.insert_all(&[TypeDefinition::new_enum("t.empty", "E", &[])]).is_err());
        assert!(r.insert_all(&[TypeDefinition::new_enum("NoDot", "E", &[("a", "A")])]).is_err());
        // Тип може посилатись на тип з тієї ж пачки
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
        assert_eq!(text::parse_value_in(&r, "Вручну", &mode), Ok(DataValue::String("manual".into())));
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
        // Старий JSON графа (без type_name) читається як звичайний об'єкт
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
