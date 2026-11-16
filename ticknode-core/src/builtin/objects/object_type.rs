use super::snapshot::{decode_fields_snapshot, encode_fields_snapshot};
use std::collections::BTreeMap;
use ticknode_shared::*;

/// Selected struct and persisted fallback shared by Make Object and Break Object.
#[derive(Default)]
pub(super) struct ObjectType {
    pub type_name: String,
    pub type_title: String,
    pub fields_snapshot: Vec<FieldDefinition>,
    pub struct_defs: BTreeMap<String, TypeDefinition>,
    pub revision: u64,
}

impl ObjectType {
    pub fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
        if name == "fields" {
            return Err(
                UiMessage::new("error-invalid-property", "Invalid property: fields")
                    .arg("property", "fields"),
            );
        }
        if name == "type" {
            let DataValue::String(s) = value else {
                return Err(
                    UiMessage::new("error-invalid-property", "Invalid property: type")
                        .arg("property", "type"),
                );
            };
            if !s.is_empty() && !self.struct_defs.contains_key(s) {
                return Err(UiMessage::new(
                    "error-object-unknown-type",
                    format!("Unknown struct type: {s}"),
                )
                .arg("type", s));
            }
        }
        Ok(())
    }

    pub fn set_property(&mut self, name: &str, value: DataValue) {
        match name {
            "type" => self.set_type(value),
            "fields" => self.restore_fields(value),
            _ => {}
        }
    }

    fn set_type(&mut self, value: DataValue) {
        let DataValue::String(s) = value else {
            return;
        };
        if s != self.type_name {
            let prev_empty = self.type_name.is_empty();
            self.type_name = s;
            if let Some(def) = self.struct_defs.get(&self.type_name) {
                self.type_title = def.title.clone();
                self.fields_snapshot = def.fields().to_vec();
            } else if self.type_name.is_empty() || !prev_empty {
                self.type_title.clear();
                self.fields_snapshot.clear();
            }
            self.revision += 1;
        }
    }

    fn restore_fields(&mut self, value: DataValue) {
        let DataValue::String(json) = value else {
            return;
        };
        let Some(snapshot) = decode_fields_snapshot(&json) else {
            return;
        };
        if !self.type_name.is_empty() && self.struct_defs.contains_key(&self.type_name) {
            return;
        }
        if self.fields_snapshot != snapshot.fields || self.type_title != snapshot.title {
            self.type_title = snapshot.title;
            self.fields_snapshot = snapshot.fields;
            self.revision += 1;
        }
    }

    pub fn restored(&mut self) {
        if self.type_name.is_empty() {
            self.type_title.clear();
            self.fields_snapshot.clear();
        }
    }

    pub fn properties(&self) -> Vec<(String, DataValue)> {
        let mut props = vec![(
            "type".to_string(),
            DataValue::String(self.type_name.clone()),
        )];
        if !self.type_name.is_empty()
            && !self.fields_snapshot.is_empty()
            && let Some(json) = encode_fields_snapshot(&self.type_title, &self.fields_snapshot)
        {
            props.push(("fields".to_string(), DataValue::String(json)));
        }
        props
    }

    pub fn types_changed(&mut self, types: &TypeRegistry) {
        let mut next_struct_defs = BTreeMap::new();
        for def in types.definitions().into_iter().filter(|d| d.is_struct()) {
            next_struct_defs.insert(def.name.clone(), def);
        }
        let structs_changed = next_struct_defs != self.struct_defs;
        self.struct_defs = next_struct_defs;

        if !self.type_name.is_empty() {
            if let Some(def) = self.struct_defs.get(&self.type_name) {
                if self.fields_snapshot != def.fields() || self.type_title != def.title {
                    self.fields_snapshot = def.fields().to_vec();
                    self.type_title = def.title.clone();
                    self.revision += 1;
                } else if structs_changed {
                    self.revision += 1;
                }
            } else if structs_changed {
                self.revision += 1;
            }
        } else if structs_changed {
            self.revision += 1;
        }
    }

    pub fn type_property(&self, description: &str) -> PropertyDefinition {
        let mut options = Vec::new();
        options.push(("".to_string(), "—".to_string()));
        if !self.type_name.is_empty() && !self.struct_defs.contains_key(&self.type_name) {
            let title = if self.type_title.is_empty() {
                self.type_name.clone()
            } else {
                self.type_title.clone()
            };
            options.push((self.type_name.clone(), title));
        }
        for (name, def) in &self.struct_defs {
            options.push((name.clone(), def.title.clone()));
        }
        let refs: Vec<_> = options
            .iter()
            .map(|(value, label)| (value.as_str(), label.as_str()))
            .collect();
        PropertyDefinition::select("type", "Type", &refs).described(description)
    }
}

#[cfg(test)]
mod tests;
