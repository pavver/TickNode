use super::{LOOKUP_OPTIONS, format_lookup, parse_lookup};
use crate::{Node, NodeContext};
use ticknode_shared::*;

pub struct VariableDeleteNode {
    pub id: NodeId,
    pub name: String,
    pub lookup: VariableLookup,
}

impl VariableDeleteNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            name: String::new(),
            lookup: VariableLookup::Auto,
        }
    }
}

impl Node for VariableDeleteNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: vec![
                PropertyDefinition::text("name", "Name"),
                PropertyDefinition::select("lookup", "Lookup", LOOKUP_OPTIONS),
            ],
            type_name: "variable/delete".to_string(),
            title: "Delete Variable".to_string(),
            category: "Variables".to_string(),
            description: "Deletes a variable by name".to_string(),
            help: "On a signal at In, deletes the resolved variable and passes execution to Out. Deleting a variable that does not exist is not an error.\n\nLookup chooses whether to search the current sheet, globally (including persistent), or automatically (sheet first, then global).".to_string(),
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
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Variable(VariableCommand::Delete {
            name: self.name.clone(),
            lookup: self.lookup,
        })]
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("name".to_string(), DataValue::String(self.name.clone())),
            (
                "lookup".to_string(),
                DataValue::String(format_lookup(self.lookup).to_string()),
            ),
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
            _ => {}
        }
    }
}
