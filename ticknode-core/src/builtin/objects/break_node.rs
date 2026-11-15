use std::collections::BTreeMap;
use ticknode_shared::*;

use crate::custom_types::field_pin;
use crate::{Node, NodeContext};

pub struct ObjectBreakNode {
    pub id: NodeId,
    pub type_name: String,
    pub type_title: String,
    pub fields_snapshot: Vec<FieldDefinition>,
    pub struct_defs: BTreeMap<String, TypeDefinition>,
    pub revision: u64,
}

impl ObjectBreakNode {
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

impl Node for ObjectBreakNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition_revision(&self) -> u64 {
        self.revision
    }

    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(0)
    }

    fn adapt_to(&mut self, pin: PinId, data_type: &DataType) -> bool {
        if pin != PinId(0) {
            return false;
        }
        let DataType::Named(name) = data_type else {
            return false;
        };
        let Some(def) = self.struct_defs.get(name) else {
            return false;
        };
        if self.type_name == *name {
            return false;
        }
        self.type_name = name.clone();
        self.type_title = def.title.clone();
        self.fields_snapshot = def.fields().to_vec();
        self.revision += 1;
        true
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
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
                self.type_name = s;
                if let Some(def) = self.struct_defs.get(&self.type_name) {
                    self.type_title = def.title.clone();
                    self.fields_snapshot = def.fields().to_vec();
                } else if self.type_name.is_empty() {
                    self.type_title.clear();
                    self.fields_snapshot.clear();
                }
                self.revision += 1;
            }
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![(
            "type".to_string(),
            DataValue::String(self.type_name.clone()),
        )]
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
                .described("Object struct type to break into fields"),
        ];

        let mut pins = Vec::new();
        if self.type_name.is_empty() {
            pins.push(PinDefinition {
                id: PinId(0),
                name: "Value".into(),
                pin_type: PinType::Data(DataType::Any),
                direction: PinDirection::Input,
                required: false,
            });
        } else {
            pins.push(PinDefinition {
                id: PinId(0),
                name: "Value".into(),
                pin_type: PinType::Data(DataType::Named(self.type_name.clone())),
                direction: PinDirection::Input,
                required: true,
            });
            pins.extend(
                self.fields_snapshot
                    .iter()
                    .enumerate()
                    .map(|(i, f)| field_pin(i, f, PinDirection::Output)),
            );
        }

        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties,
            type_name: "object/break".into(),
            title: "Break Object".into(),
            category: "Data".into(),
            description: "Breaks a typed object into individual fields".into(),
            help: "Breaks a typed object into individual fields: each field is an output pin.\n\nSelect Type on the node or connect a typed object to Value to adapt it.\n\nAn optional field that is absent from the value has no value (the pin is empty).\n\nPure node: evaluated whenever its result is needed by another node.".into(),
            pins,
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let Some(DataValue::Object { fields, .. }) = ctx.get_value(PinId(0)) else {
            return Vec::new();
        };
        self.fields_snapshot
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                Some(NodeOutput::Data(
                    PinId(i as u32 + 1),
                    fields.get(&f.name)?.value.clone(),
                ))
            })
            .collect()
    }
}
