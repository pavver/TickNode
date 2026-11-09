use ticknode_shared::*;
use crate::{Node, NodeContext};
use super::{format_lookup, parse_lookup, LOOKUP_OPTIONS};

pub struct VariableGetNode {
    pub id: NodeId,
    pub name: String,
    pub lookup: VariableLookup,
    pub value_type: DataType,
}

impl VariableGetNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            name: String::new(),
            lookup: VariableLookup::Auto,
            value_type: DataType::Any,
        }
    }
}

impl Node for VariableGetNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: vec![
                PropertyDefinition::text("name", "Name"),
                PropertyDefinition::select("lookup", "Lookup", LOOKUP_OPTIONS),
            ],
            type_name: "variable/get".to_string(),
            title: "Get Variable".to_string(),
            category: "Variables".to_string(),
            description: "Returns the value of a variable by name".to_string(),
            help: "Reads the variable named in the \"Name\" property and outputs its value on Value, indicating on Exists whether it was found.\n\nLookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Value".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Exists".to_string(),
                    pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if let Some(var) = ctx.variable(&self.name, self.lookup) {
            if let Some(val) = fit_variable_value(ctx.types, var.value.clone(), &self.value_type) {
                return vec![
                    NodeOutput::Data(PinId(0), val),
                    NodeOutput::Data(PinId(1), DataValue::Bool(true)),
                ];
            }
        }
        vec![
            NodeOutput::Clear(PinId(0)),
            NodeOutput::Data(PinId(1), DataValue::Bool(false)),
        ]
    }

    fn adapt_to(&mut self, pin: PinId, data_type: &DataType) -> bool {
        if pin != PinId(0) {
            return false;
        }
        if &self.value_type == data_type {
            return false;
        }
        self.value_type = data_type.clone();
        true
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("name".to_string(), DataValue::String(self.name.clone())),
            ("lookup".to_string(), DataValue::String(format_lookup(self.lookup).to_string())),
            ("value_type".to_string(), DataValue::String(data_type_name(&self.value_type))),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        match name {
            "name" => {
                if let DataValue::String(s) = value {
                    self.name = s;
                }
            }
            "lookup" => {
                if let DataValue::String(s) = value {
                    self.lookup = parse_lookup(&s);
                }
            }
            "value_type" => {
                if let DataValue::String(s) = value {
                    self.value_type = parse_data_type(&s).unwrap_or(DataType::Any);
                }
            }
            "var_name" => {
                // Legacy migration
                if let DataValue::String(s) = value {
                    self.name = s;
                    self.lookup = VariableLookup::Auto;
                    self.value_type = DataType::Number;
                }
            }
            _ => {}
        }
    }
}
