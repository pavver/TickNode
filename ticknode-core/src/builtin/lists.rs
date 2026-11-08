use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ListCreateNode {
    pub id: NodeId,
    pub element_type: DataType,
}
impl Node for ListCreateNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 1,
            properties: Vec::new(),
            type_name: "list/create".to_string(),
            title: "Create List".to_string(),
            category: "Lists".to_string(),
            description: "Creates an empty list".to_string(),
            help: "Outputs an empty list. The element type is selected on the node.\n\nPopulate the list using the List Add node.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "List".to_string(),
                    pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))),
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(
            PinId(0),
            DataValue::List {
                element_type: self.element_type.clone(),
                elements: Vec::new(),
            }
        )]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.element_type, "element_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListAddNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListAddNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 5,
            properties: Vec::new(),
            type_name: "list/add".to_string(),
            title: "List Add".to_string(),
            category: "Lists".to_string(),
            description: "Appends an item to a list".to_string(),
            help: "Outputs OutList — a copy of List with Item appended at the end. The input list is not modified.\n\nThe element type is selected on the node and must match the list type.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let item_val = ctx.get_value(PinId(1));
        if let (Some(DataValue::List { element_type, mut elements }), Some(item)) = (list_val, item_val) {
            elements.push(item);
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.element_type, "element_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListRemoveNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListRemoveNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 5,
            properties: Vec::new(),
            type_name: "list/remove".to_string(),
            title: "List Remove".to_string(),
            category: "Lists".to_string(),
            description: "Removes an item from a list by index".to_string(),
            help: "Outputs OutList — a copy of the list without the element at Index (0-based). If the index is out of bounds, the list is returned unchanged.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "OutList".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { element_type, mut elements }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                elements.remove(index_val as usize);
            }
            vec![NodeOutput::Data(PinId(2), DataValue::List { element_type, elements })]
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.element_type, "element_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListGetNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 1,
            properties: Vec::new(),
            type_name: "list/get".to_string(),
            title: "List Get".to_string(),
            category: "Lists".to_string(),
            description: "Gets an item from a list by index".to_string(),
            help: "Outputs Item — the list element at Index (0-based). If the index is out of bounds, the output has no value.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Item".to_string(), pin_type: PinType::Data(self.element_type.clone()), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        let index_val = ctx.get_number(PinId(1), NumberValue::Int(-1)).to_f64() as i64;
        if let Some(DataValue::List { elements, .. }) = list_val {
            if index_val >= 0 && index_val < elements.len() as i64 {
                vec![NodeOutput::Data(PinId(2), elements[index_val as usize].clone())]
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.element_type, "element_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}

pub struct ListLengthNode { pub id: NodeId, pub element_type: DataType }
impl Node for ListLengthNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 1,
            properties: Vec::new(),
            type_name: "list/length".to_string(),
            title: "List Length".to_string(),
            category: "Lists".to_string(),
            description: "Returns the length of a list".to_string(),
            help: "Outputs Len — the number of elements in the list. If no list is connected, outputs 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "List".to_string(), pin_type: PinType::Data(DataType::List(Box::new(self.element_type.clone()))), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Len".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let list_val = ctx.get_value(PinId(0));
        if let Some(DataValue::List { elements, .. }) = list_val {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(elements.len() as i64)))]
        } else {
            vec![NodeOutput::Data(PinId(1), DataValue::Number(NumberValue::Int(0)))]
        }
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        super::properties::set_type_property(&mut self.element_type, "element_type", name, value);
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("element_type".to_string(), DataValue::String(data_type_name(&self.element_type)))]
    }
}
