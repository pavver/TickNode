use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct LogicalNode { pub id: NodeId, pub op: LogOp }
impl Node for LogicalNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let (name, title) = operation_identity(self.op);
        let pins = match self.op {
            LogOp::And | LogOp::Or => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
            LogOp::Not => vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        };
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Logic".to_string(),
            description: "Logical operation".to_string(),
            help: match self.op {
                LogOp::And => "Res = true only if both A and B are true.",
                LogOp::Or => "Res = true if at least one of A or B is true.",
                LogOp::Not => "Res is the logical negation of A.",
            }.to_string() + "\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to false.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.",
            pins,
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_bool(PinId(0), false);
        let b = ctx.get_bool(PinId(1), false);
        let res = match self.op { LogOp::And => a && b, LogOp::Or => a || b, LogOp::Not => !a };
        vec![NodeOutput::Data(PinId(2), DataValue::Bool(res))]
    }
}

pub struct CompareNode { pub id: NodeId }
impl Node for CompareNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "compare".to_string(),
            title: "Compare".to_string(),
            category: "Logic".to_string(),
            description: "Compares two numbers".to_string(),
            help: "Compares numbers A and B:\n• Eq — A equals B (within 1e-9);\n• Gt — A is greater than B;\n• Lt — A is less than B.\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0.\n\nFor branching execution on a condition, the Match node is more convenient.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Eq".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(3), name: "Gt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(4), name: "Lt".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0)).to_f64();
        let b = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64();
        vec![
            NodeOutput::Data(PinId(2), DataValue::Bool((a - b).abs() < 1e-9)),
            NodeOutput::Data(PinId(3), DataValue::Bool(a > b)),
            NodeOutput::Data(PinId(4), DataValue::Bool(a < b)),
        ]
    }
}

fn operation_identity(op: LogOp) -> (&'static str, &'static str) {
    match op {
        LogOp::And => ("logical/and", "Logical And"),
        LogOp::Or => ("logical/or", "Logical Or"),
        LogOp::Not => ("logical/not", "Logical Not"),
    }
}
