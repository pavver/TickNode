use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct VariableGetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableGetNode {
    fn id(&self) -> NodeId { self.id }
    fn is_pure(&self) -> bool { true }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "variable/get".to_string(),
            title: format!("Get {}", self.var_name),
            category: "Data".to_string(),
            description: "Returns the value of a variable by name".to_string(),
            help: "Reads the global variable named in the \"Variable\" property and outputs it on Val. Variables are shared across the entire graph and written by the Set node. Until the variable is written, outputs 0.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(DataType::Number),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_var(&self.var_name).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Data(PinId(0), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}

pub struct VariableSetNode { pub id: NodeId, pub var_name: String }
impl Node for VariableSetNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "variable/set".to_string(),
            title: format!("Set {}", self.var_name),
            category: "Data".to_string(),
            description: "Writes a value to a variable".to_string(),
            help: "On a signal at In, writes the value from Val to the global variable (named in the \"Variable\" property) and passes execution to Out.\n\nThe variable is visible to all Get nodes with the same name across the entire graph.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(0), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _:Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1)).unwrap_or(DataValue::Number(NumberValue::Int(0)));
        vec![NodeOutput::Execution(PinId(0)), NodeOutput::Variable(self.var_name.clone(), val)]
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("var_name".to_string(), DataValue::String(self.var_name.clone()))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "var_name" {
            if let DataValue::String(s) = value {
                self.var_name = s;
            }
        }
    }
}
