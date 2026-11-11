use ticknode_shared::*;
use crate::{Node, NodeContext};
use super::{format_scope, parse_scope, SCOPE_OPTIONS};

pub struct VariableCreateNode {
    pub id: NodeId,
    pub name: String,
    pub scope: VariableScope,
    pub value_type: DataType,
}

impl VariableCreateNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            name: String::new(),
            scope: VariableScope::Sheet,
            value_type: DataType::Any,
        }
    }
}

impl Node for VariableCreateNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: vec![
                PropertyDefinition::text("name", "Name"),
                PropertyDefinition::select("scope", "Scope", SCOPE_OPTIONS),
            ],
            type_name: "variable/create".to_string(),
            title: "Create Variable".to_string(),
            category: "Variables".to_string(),
            description: "Creates a new variable if it does not already exist".to_string(),
            help: "On a signal at In, creates a new variable in the chosen scope with the initial value from Initial (defaulting to the type's default value if unconnected).\n\nIf the variable already existed in that scope, execution passes to Exists without modifying its value; otherwise execution passes to Created.".to_string(),
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
                    name: "Created".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Exists".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(3),
                    name: "Initial".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: false,
                },
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value = ctx.get_value(PinId(3)).unwrap_or_else(|| ctx.types.default_value(&self.value_type));
        vec![NodeOutput::Variable(VariableCommand::Create {
            name: self.name.clone(),
            scope: self.scope,
            data_type: self.value_type.clone(),
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
            ("scope".to_string(), DataValue::String(format_scope(self.scope).to_string())),
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
            "scope" => {
                if let DataValue::String(s) = value {
                    self.scope = parse_scope(&s);
                }
            }
            "value_type" => {
                if let DataValue::String(s) = value {
                    self.value_type = parse_data_type(&s).unwrap_or(DataType::Any);
                }
            }
            _ => {}
        }
    }
}
