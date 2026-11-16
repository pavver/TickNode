use std::collections::BTreeMap;
use ticknode_shared::*;

use crate::custom_types::field_pin;
use crate::{Node, NodeContext};

use super::object_type::ObjectType;

pub struct ObjectMakeNode {
    pub id: NodeId,
    object: ObjectType,
}

impl ObjectMakeNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            object: ObjectType::default(),
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
        self.object.revision
    }

    fn validate_property(&self, name: &str, value: &DataValue) -> Result<(), UiMessage> {
        self.object.validate_property(name, value)
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        self.object.set_property(name, value)
    }

    fn restored(&mut self) {
        self.object.restored()
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        self.object.properties()
    }

    fn types_changed(&mut self, types: &TypeRegistry) {
        self.object.types_changed(types)
    }

    fn definition(&self) -> NodeDefinition {
        let properties = vec![
            self.object
                .type_property("Object struct type to construct from fields"),
            PropertyDefinition::hidden("fields"),
        ];

        let mut pins = Vec::new();
        let out_pin_type = if self.object.type_name.is_empty() {
            PinType::Data(DataType::Any)
        } else {
            PinType::Data(DataType::Named(self.object.type_name.clone()))
        };
        pins.push(PinDefinition {
            id: PinId(0),
            name: "Value".into(),
            pin_type: out_pin_type,
            direction: PinDirection::Output,
            required: false,
        });
        if !self.object.type_name.is_empty() {
            pins.extend(
                self.object
                    .fields_snapshot
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
        if self.object.type_name.is_empty() {
            return Vec::new();
        }
        let mut fields = BTreeMap::new();
        for (i, f) in self.object.fields_snapshot.iter().enumerate() {
            if let Some(value) = ctx.get_value(PinId(i as u32 + 1)) {
                fields.insert(
                    f.name.clone(),
                    ObjectField {
                        data_type: effective_type(
                            Some(&value),
                            if matches!(f.data_type, DataType::Any | DataType::Object) {
                                ctx.input_types
                                    .get(&PinId(i as u32 + 1))
                                    .unwrap_or(&f.data_type)
                            } else {
                                &f.data_type
                            },
                            "",
                            ctx.types,
                        ),
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
                type_name: Some(self.object.type_name.clone()),
            },
        )]
    }
}
