use std::collections::BTreeMap;
use ticknode_shared::*;

use crate::custom_types::field_pin;
use crate::{Node, NodeContext};

use super::snapshot::{decode_fields_snapshot, encode_fields_snapshot};

pub struct ObjectMakeNode {
    pub id: NodeId,
    pub type_name: String,
    pub type_title: String,
    pub fields_snapshot: Vec<FieldDefinition>,
    pub struct_defs: BTreeMap<String, TypeDefinition>,
    pub revision: u64,
}

impl ObjectMakeNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            type_name: String::new(),
            type_title: String::new(),
            fields_snapshot: Vec::new(),
            struct_defs: BTreeMap::new(),
            revision: 0,
        }
    }
}

impl Node for ObjectMakeNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition_revision(&self) -> u64 {
        self.revision
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
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

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "type" {
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
        } else if name == "fields" {
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
    }

    fn restored(&mut self) {
        if self.type_name.is_empty() {
            self.type_title.clear();
            self.fields_snapshot.clear();
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
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

    fn types_changed(&mut self, types: &TypeRegistry) {
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

    fn definition(&self) -> NodeDefinition {
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
        let options_ref: Vec<(&str, &str)> = options
            .iter()
            .map(|(v, l)| (v.as_str(), l.as_str()))
            .collect();
        let properties = vec![
            PropertyDefinition::select("type", "Type", &options_ref)
                .described("Object struct type to construct from fields"),
            PropertyDefinition::hidden("fields"),
        ];

        let mut pins = Vec::new();
        let out_pin_type = if self.type_name.is_empty() {
            PinType::Data(DataType::Any)
        } else {
            PinType::Data(DataType::Named(self.type_name.clone()))
        };
        pins.push(PinDefinition {
            id: PinId(0),
            name: "Value".into(),
            pin_type: out_pin_type,
            direction: PinDirection::Output,
            required: false,
        });
        if !self.type_name.is_empty() {
            pins.extend(
                self.fields_snapshot
                    .iter()
                    .enumerate()
                    .map(|(i, f)| field_pin(i, f, PinDirection::Input)),
            );
        }

        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties,
            type_name: "object/make".into(),
            title: "Make Object".into(),
            category: "Data".into(),
            description: "Constructs a typed object from individual fields".into(),
            help: "Constructs a typed object from individual fields.\n\nSelect Type on the node to choose the object structure.\n\nRequired fields are required inputs: without them no value is created. An optional field without a value is skipped.\n\nPure node: evaluated whenever its result is needed by another node.".into(),
            pins,
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if self.type_name.is_empty() {
            return Vec::new();
        }
        let mut fields = BTreeMap::new();
        for (i, f) in self.fields_snapshot.iter().enumerate() {
            if let Some(value) = ctx.get_value(PinId(i as u32 + 1)) {
                fields.insert(
                    f.name.clone(),
                    ObjectField {
                        name: f.name.clone(),
                        description: f.description.clone(),
                        value,
                    },
                );
            } else if !f.optional {
                return Vec::new();
            }
        }
        vec![NodeOutput::Data(
            PinId(0),
            DataValue::Object {
                fields,
                type_name: Some(self.type_name.clone()),
            },
        )]
    }
}
