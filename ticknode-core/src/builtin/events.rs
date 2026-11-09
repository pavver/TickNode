use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ClickButtonNode {
    pub id: NodeId,
    pub label: String,
}

impl Node for ClickButtonNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            cost: 1,
            properties: Vec::new(),
            type_name: "test/click_button".to_string(),
            title: "Click Button".to_string(),
            category: "Events".to_string(),
            description: "Emits an exec signal on the next tick after the user clicks the button".to_string(),
            help: "Emits a signal on Out when you click the Run button on the node.\n\nHandy for manually triggering a chain while debugging.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Out".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output, required: false }
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "label" {
            if let DataValue::String(s) = value {
                self.label = s;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("label".to_string(), DataValue::String(self.label.clone()))]
    }
}
