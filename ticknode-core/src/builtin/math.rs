use crate::{Node, NodeContext};
use ticknode_shared::*;

/// `value_type` is Number or a physical quantity: Add/Sub keep it on A, B and Res; Mul/Div scale A by a plain B.
pub struct PureMathNode {
    pub id: NodeId,
    pub op: MathOp,
    pub value_type: DataType,
}
impl PureMathNode {
    pub fn new(id: NodeId, op: MathOp) -> Self {
        Self {
            id,
            op,
            value_type: DataType::Number,
        }
    }
    fn same_type_b(&self) -> bool {
        matches!(self.op, MathOp::Add | MathOp::Sub)
    }
}

/// Number or a core physical quantity (canonical numbers on the wire).
pub(crate) fn numeric_type(t: &DataType) -> bool {
    match t {
        DataType::Number => true,
        DataType::Named(name) => quantity::kind(name).is_some(),
        _ => false,
    }
}

impl Node for PureMathNode {
    fn id(&self) -> NodeId {
        self.id
    }
    fn is_pure(&self) -> bool {
        true
    }
    fn definition(&self) -> NodeDefinition {
        let (name, title) = operation_identity(self.op);
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: Vec::new(),
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Math".to_string(),
            description: "Arithmetic operation on two numbers".to_string(),
            help: match self.op {
                MathOp::Add => "Result = A + B.",
                MathOp::Sub => "Result = A − B.",
                MathOp::Mul => "Result = A × B.",
                MathOp::Div => "Result = A ÷ B. Division by 0 yields infinity.",
            }
            .to_string()
                + "\n\nConnecting a physical quantity keeps its type: W + W gives W, and for Multiply/Divide A × number keeps A's quantity (B stays a plain number). Values are canonical numbers, so mixed display units add correctly.\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.",
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "A".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "B".to_string(),
                    pin_type: PinType::Data(if self.same_type_b() {
                        self.value_type.clone()
                    } else {
                        DataType::Number
                    }),
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Result".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0));
        let b = ctx.get_number(PinId(1), NumberValue::Int(0));
        let res = match self.op {
            MathOp::Add => a + b,
            MathOp::Sub => a - b,
            MathOp::Mul => a * b,
            MathOp::Div => a / b,
        };
        vec![NodeOutput::Data(PinId(2), DataValue::Number(res))]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        // Plain numbers persist nothing, so existing graphs stay unchanged.
        if self.value_type == DataType::Number {
            return Vec::new();
        }
        vec![("value_type".into(), data_type_name(&self.value_type).into())]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value_type"
            && let DataValue::String(s) = value
            && let Some(t) = parse_data_type(&s).filter(numeric_type)
        {
            self.value_type = t;
        }
    }
    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        let adapts = pin == PinId(0) || pin == PinId(2) || (pin == PinId(1) && self.same_type_b());
        if !adapts || !numeric_type(t) || *t == self.value_type {
            return false;
        }
        self.value_type = t.clone();
        true
    }
    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(0) || (pin == PinId(1) && self.same_type_b())
    }
}

mod expression;
pub use expression::*;

fn operation_identity(op: MathOp) -> (&'static str, &'static str) {
    match op {
        MathOp::Add => ("math/add", "Math Add"),
        MathOp::Sub => ("math/sub", "Math Subtract"),
        MathOp::Mul => ("math/mul", "Math Multiply"),
        MathOp::Div => ("math/div", "Math Divide"),
    }
}
