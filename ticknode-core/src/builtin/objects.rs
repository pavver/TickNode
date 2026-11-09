use ticknode_shared::*;
use crate::{Node, NodeContext};

// Helper to parse fields_list
fn parse_fields_list(s: &str) -> Vec<(String, DataType)> {
    let mut fields = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() { continue; }
        if let Some((name, ty_str)) = part.split_once(':') {
            let name = name.trim().to_string();
            let ty = parse_data_type(ty_str).unwrap_or(DataType::String);
            fields.push((name, ty));
        } else {
            fields.push((part.to_string(), DataType::String));
        }
    }
    fields
}

pub struct ObjectCreateNode { pub id: NodeId }
impl Node for ObjectCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "object/create".to_string(),
            title: "Create Object".to_string(),
            category: "Objects".to_string(),
            description: "Creates an empty object".to_string(),
            help: "Outputs an empty object — a collection of named fields.\n\nAdd fields using the Set Object Field node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), DataValue::Object { fields: std::collections::BTreeMap::new(), type_name: None })]
    }
}

pub struct ObjectSetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectSetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "object/set_field".to_string(),
            title: "Set Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Sets the value of an object field".to_string(),
            help: "Outputs OutObj — a copy of object Obj where field Name has value Val. If the field did not exist, it is added; otherwise it is replaced.\n\nThe type of Val is selected on the node. Until all inputs are provided, no output is produced.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: {
                let mut pins = field_inputs();
                pins.extend([
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "OutObj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Output, required: false },
                ]);
                pins
            },
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        let val = ctx.get_value(PinId(2));
        if let (Some(DataValue::Object { mut fields, .. }), Some(DataValue::String(name)), Some(v)) = (obj_val, name_val, val) {
            fields.insert(name.clone(), ObjectField {
                name,
                description: String::new(),
                value: v,
            });
            vec![NodeOutput::Data(PinId(3), DataValue::Object { fields, type_name: None })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.field_type, "field_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectGetFieldNode { pub id: NodeId, pub field_type: DataType }
impl Node for ObjectGetFieldNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "object/get_field".to_string(),
            title: "Get Object Field".to_string(),
            category: "Objects".to_string(),
            description: "Gets the value of an object field".to_string(),
            help: "Looks up field Name in object Obj:\n• Val — the field value;\n• Found — whether the field exists.\n\nThe type of Val is selected on the node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: {
                let mut pins = field_inputs();
                pins.extend([
                PinDefinition { id: PinId(2), name: "Val".to_string(), pin_type: PinType::Data(self.field_type.clone()), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Found".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                ]);
                pins
            },
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let name_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::Object { fields, .. }), Some(DataValue::String(name))) = (obj_val, name_val) {
            if let Some(field) = fields.get(&name) {
                vec![
                    NodeOutput::Data(PinId(2), field.value.clone()),
                    NodeOutput::Data(PinId(3), DataValue::Bool(true)),
                ]
            } else {
                vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
            }
        } else {
            vec![NodeOutput::Data(PinId(3), DataValue::Bool(false))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.field_type, "field_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("field_type".to_string(), DataValue::String(data_type_name(&self.field_type)))]
    }
}

pub struct ObjectUnpackNode {
    pub id: NodeId,
    pub fields_list: String,
    pub parsed_fields: Vec<(String, DataType)>,
}
impl Node for ObjectUnpackNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![
            PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
        ];
        for (i, (name, ty)) in self.parsed_fields.iter().enumerate() {
            pins.push(PinDefinition {
                id: PinId((i + 1) as u32),
                name: name.clone(),
                pin_type: PinType::Data(ty.clone()),
                direction: PinDirection::Output, required: false });
        }
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "object/unpack".to_string(),
            title: "Unpack Object".to_string(),
            category: "Objects".to_string(),
            description: "Unpacks fields of an object".to_string(),
            help: "Outputs several object fields at once. Fields are listed on the node or in the right panel separated by commas as \"name:type\", for example `temp:number, city:string` (without a type — string).\n\nEach field becomes a separate output pin. A field missing from the object has no value.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let obj_val = ctx.get_value(PinId(0));
        let mut outputs = Vec::new();
        if let Some(DataValue::Object { fields, .. }) = obj_val {
            for (i, (name, _)) in self.parsed_fields.iter().enumerate() {
                if let Some(field) = fields.get(name) {
                    outputs.push(NodeOutput::Data(PinId((i + 1) as u32), field.value.clone()));
                }
            }
        }
        outputs
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "fields_list" {
            if let DataValue::String(s) = value {
                self.fields_list = s;
                self.parsed_fields = parse_fields_list(&self.fields_list);
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("fields_list".to_string(), DataValue::String(self.fields_list.clone()))]
    }
}

fn field_inputs() -> Vec<PinDefinition> {
    vec![
        PinDefinition { id: PinId(0), name: "Obj".to_string(), pin_type: PinType::Data(DataType::Object), direction: PinDirection::Input, required: false },
        PinDefinition { id: PinId(1), name: "Name".to_string(), pin_type: PinType::Data(DataType::String), direction: PinDirection::Input, required: false },
    ]
}
