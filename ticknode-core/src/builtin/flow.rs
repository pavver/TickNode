use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ForLoopNode {
    pub id: NodeId,
    pub active: bool,
    pub current_index: i64,
    pub count: i64,
}
impl Node for ForLoopNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "for_loop".to_string(),
            title: "For Loop".to_string(),
            category: "Control Flow".to_string(),
            description: "Asynchronous loop from 0 to Count-1 with step 1".to_string(),
            help: "Step-by-step loop:\n1. A signal at In starts the loop with Index = 0 and fires LoopBody.\n2. When the loop body finishes, send a signal to Next — Index increments by 1 and LoopBody fires again.\n3. After Count iterations or a signal at Break, Completed fires.\n\nIf Count ≤ 0, Completed fires immediately. Each step takes at least one tick, so the loop never blocks the engine.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Count".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Break".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "Next".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "LoopBody".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(5), name: "Index".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(6), name: "Completed".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let count_val = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64() as i64;
            if count_val <= 0 {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                self.active = true;
                self.current_index = 0;
                self.count = count_val;
                vec![
                    NodeOutput::Execution(PinId(4)),
                    NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(0))),
                ]
            }
        } else if trigger_pin == Some(PinId(2)) {
            if self.active {
                self.active = false;
                vec![NodeOutput::Execution(PinId(6))]
            } else {
                vec![]
            }
        } else if trigger_pin == Some(PinId(3)) {
            if self.active {
                self.current_index += 1;
                if self.current_index >= self.count {
                    self.active = false;
                    vec![NodeOutput::Execution(PinId(6))]
                } else {
                    vec![
                        NodeOutput::Execution(PinId(4)),
                        NodeOutput::Data(PinId(5), DataValue::Number(NumberValue::Int(self.current_index))),
                    ]
                }
            } else {
                vec![]
            }
        } else {
            vec![]
        }
    }
}
