use ticknode_shared::*;

use crate::custom_types::field_pin;
use crate::{Node, NodeContext};

use super::object_type::ObjectType;

pub struct ObjectBreakNode {
    pub id: NodeId,
    object: ObjectType,
}

impl ObjectBreakNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            object: ObjectType::default(),
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
        self.object.revision
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
        let Some(def) = self.object.struct_defs.get(name) else {
            return false;
        };
        if self.object.type_name == *name {
            return false;
        }
        self.object.type_name = name.clone();
        self.object.type_title = def.title.clone();
        self.object.fields_snapshot = def.fields().to_vec();
        self.object.revision += 1;
        true
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
                .type_property("Object struct type to break into fields"),
            PropertyDefinition::hidden("fields"),
        ];

        let mut pins = Vec::new();
        if self.object.type_name.is_empty() {
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
                pin_type: PinType::Data(DataType::Named(self.object.type_name.clone())),
                direction: PinDirection::Input,
                required: true,
            });
            pins.extend(
                self.object
                    .fields_snapshot
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
        self.object
            .fields_snapshot
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
