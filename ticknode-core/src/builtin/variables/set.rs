use ticknode_shared::*;
use crate::{Node, NodeContext};
use super::{format_lookup, parse_lookup, LOOKUP_OPTIONS};

pub struct VariableSetNode {
    pub id: NodeId,
    pub name: String,
    pub lookup: VariableLookup,
    pub value_type: DataType,
}

impl VariableSetNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            name: String::new(),
            lookup: VariableLookup::Auto,
            value_type: DataType::Any,
        }
    }
}

impl Node for VariableSetNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: vec![
                PropertyDefinition::text("name", "Name"),
                PropertyDefinition::select("lookup", "Lookup", LOOKUP_OPTIONS),
            ],
            type_name: "variable/set".to_string(),
            title: "Set Variable".to_string(),
            category: "Variables".to_string(),
            description: "Writes a value to a variable".to_string(),
            help: "On a signal at In, writes the value from Value to the resolved variable and passes execution to Out. If the variable is missing or the value does not fit its type, execution passes to Failed and an error is recorded.\n\nLookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "In".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Out".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Failed".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(3),
                    name: "Value".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: true,
                },
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value = ctx.get_value(PinId(3)).unwrap_or_else(|| text::default_of(&self.value_type));
        vec![NodeOutput::Variable(VariableCommand::Set {
            name: self.name.clone(),
            lookup: self.lookup,
            value,
        })]
    }

    fn adapt_to(&mut self, pin: PinId, data_type: &DataType) -> bool {
        if pin != PinId(3) {
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
