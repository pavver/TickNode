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

/// `value_type` is Number or a physical quantity shared by A and B (canonical numbers are compared).
pub struct CompareNode { pub id: NodeId, pub value_type: DataType }
impl CompareNode {
    pub fn new(id: NodeId) -> Self { Self { id, value_type: DataType::Number } }
}
impl Node for CompareNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: Vec::new(),
            type_name: "compare".to_string(),
            title: "Compare".to_string(),
            category: "Logic".to_string(),
            description: "Compares two numbers".to_string(),
            help: "Compares numbers A and B:\n• Eq — A equals B (within 1e-9);\n• Gt — A is greater than B;\n• Lt — A is less than B.\n\nConnecting a physical quantity (for example temperature) gives both inputs its type, so B can be entered in any of its units (20 °C or 68 °F).\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0.\n\nFor branching execution on a condition, the Match node is more convenient.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(self.value_type.clone()), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(self.value_type.clone()), direction: PinDirection::Input, required: false },
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
    fn properties(&self) -> Vec<(String, DataValue)> {
        // Plain numbers persist nothing, so existing graphs stay unchanged.
        if self.value_type == DataType::Number { return Vec::new(); }
        vec![("value_type".into(), data_type_name(&self.value_type).into())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value_type" && let DataValue::String(s) = value
            && let Some(t) = parse_data_type(&s).filter(super::math::numeric_type)
        {
            self.value_type = t;
        }
    }
    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        if pin.0 > 1 || !super::math::numeric_type(t) || *t == self.value_type { return false; }
        self.value_type = t.clone();
        true
    }
    fn adapts_on_connect(&self, pin: PinId) -> bool { pin.0 <= 1 }
}

fn operation_identity(op: LogOp) -> (&'static str, &'static str) {
    match op {
        LogOp::And => ("logical/and", "Logical And"),
        LogOp::Or => ("logical/or", "Logical Or"),
        LogOp::Not => ("logical/not", "Logical Not"),
    }
}
