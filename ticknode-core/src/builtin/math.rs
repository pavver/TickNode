use std::collections::HashMap;
use evalexpr::*;
use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct PureMathNode { pub id: NodeId, pub op: MathOp }
impl Node for PureMathNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let (name, title) = operation_identity(self.op);
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: name.to_string(),
            title: title.to_string(),
            category: "Math".to_string(),
            description: "Arithmetic operation on two numbers".to_string(),
            help: match self.op {
                MathOp::Add => "Res = A + B.",
                MathOp::Sub => "Res = A − B.",
                MathOp::Mul => "Res = A × B.",
                MathOp::Div => "Res = A ÷ B. Division by 0 yields infinity.",
            }.to_string() + "\n\nAn unconnected input uses the value set next to the pin on the node, or defaults to 0. An integer stays an integer if the result is whole.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.",
            pins: vec![
                PinDefinition { id: PinId(0), name: "A".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "B".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Res".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a = ctx.get_number(PinId(0), NumberValue::Int(0));
        let b = ctx.get_number(PinId(1), NumberValue::Int(0));
        let res = match self.op {
            MathOp::Add => a + b, MathOp::Sub => a - b, MathOp::Mul => a * b, MathOp::Div => a / b,
        };
        vec![NodeOutput::Data(PinId(2), DataValue::Number(res))]
    }
}

pub struct MathExpressionNode {
    pub id: NodeId,
    pub expression: String,
    pub variable_mapping: HashMap<String, PinId>,
}
impl Node for MathExpressionNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        let mut pins = vec![PinDefinition {
            id: PinId(0),
            name: "Res".to_string(),
            pin_type: PinType::Data(DataType::Number),
            direction: PinDirection::Output, required: false }];
        let mut vars: Vec<_> = self.variable_mapping.iter().collect();
        vars.sort_by_key(|(_, pin)| pin.0);
        for (name, pin_id) in vars {
            pins.push(PinDefinition {
                id: *pin_id,
                name: name.clone(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Input, required: false });
        }
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "math/expression".to_string(),
            title: "Expression".to_string(),
            category: "Math".to_string(),
            description: "Evaluates an arbitrary mathematical formula".to_string(),
            help: "Evaluates the formula set directly on the node (\"Formula\" row), such as `(a + b) / 2` or `x * x - 1`.\n\nEach variable in the formula automatically becomes an input pin (in alphabetical order). An unconnected variable uses the value set next to the pin, or defaults to 0. If the formula is invalid, the result is 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins,
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let mut context = HashMapContext::new();
        for (name, pin) in &self.variable_mapping {
            let val = ctx.get_number(*pin, NumberValue::Int(0));
            let _ = context.set_value(name.clone().into(), Value::from(val.to_f64()));
        }
        let result = match eval_with_context(&self.expression, &context) {
            Ok(val) => {
                if let Ok(f) = val.as_float() { NumberValue::Float(f).normalize() }
                else if let Ok(i) = val.as_int() { NumberValue::Int(i).normalize() }
                else { NumberValue::Int(0) }
            }
            Err(_) => NumberValue::Int(0),
        };
        vec![NodeOutput::Data(PinId(0), DataValue::Number(result))]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("expression".to_string(), DataValue::String(self.expression.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "expression" {
            if let DataValue::String(s) = value {
                self.variable_mapping = expression_variables(&s);
                self.expression = s;
            }
        }
    }
}

/// Formula variables become input pins (1, 2, ...) in alphabetical order.
pub fn expression_variables(expression: &str) -> HashMap<String, PinId> {
    let mut names: Vec<String> = match build_operator_tree(expression) {
        Ok(tree) => tree.iter_read_variable_identifiers().map(str::to_string).collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names.dedup();
    names.into_iter().enumerate().map(|(i, n)| (n, PinId(i as u32 + 1))).collect()
}

fn operation_identity(op: MathOp) -> (&'static str, &'static str) {
    match op {
        MathOp::Add => ("math/add", "Math Add"),
        MathOp::Sub => ("math/sub", "Math Subtract"),
        MathOp::Mul => ("math/mul", "Math Multiply"),
        MathOp::Div => ("math/div", "Math Divide"),
    }
}
