use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ConstNode { pub id: NodeId, pub value: DataValue }
impl Node for ConstNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            properties: Vec::new(),
            type_name: "const".to_string(),
            title: "Constant".to_string(),
            category: "Data".to_string(),
            description: "Returns a constant value".to_string(),
            help: "Emits the value set directly on the node (\"Value\" row) or in the right panel on output Val. The value type (number, string, boolean, etc.) is chosen on the node, and the pin type follows it.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(self.value.get_type()),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, _:&mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), self.value.clone())]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("value".to_string(), self.value.clone())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value" {
            self.value = value;
        }
    }
}
