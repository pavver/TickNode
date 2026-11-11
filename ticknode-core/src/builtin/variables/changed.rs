use ticknode_shared::*;
use crate::{HostState, Node, NodeContext};
use super::{format_lookup, parse_lookup, LOOKUP_OPTIONS};

pub struct VariableChangedNode {
    pub id: NodeId,
    pub name: String,
    pub lookup: VariableLookup,
    pub value_type: DataType,
    pub sheet: Option<SheetId>,
}

impl VariableChangedNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            name: String::new(),
            lookup: VariableLookup::Auto,
            value_type: DataType::Any,
            sheet: None,
        }
    }
}

impl Node for VariableChangedNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn set_sheet(&mut self, sheet: Option<SheetId>) {
        self.sheet = sheet;
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: true,
            cost: 1,
            properties: vec![
                PropertyDefinition::text("name", "Name"),
                PropertyDefinition::select("lookup", "Lookup", LOOKUP_OPTIONS),
            ],
            type_name: "variable/changed".to_string(),
            title: "Variable Changed".to_string(),
            category: "Variables".to_string(),
            description: "Fires when a variable is created or updated with a new value".to_string(),
            help: "Triggers execution on Changed whenever the resolved variable is created or set to a different value. Deletion does not fire. Value outputs the new value.\n\nLookup chooses whether to watch the current sheet, globally (including persistent), or automatically (sheet first, then global).".to_string(),
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Changed".to_string(),
                    pin_type: PinType::Exec,
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Value".to_string(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
        }
    }

    fn polls(&self) -> bool {
        true
    }

    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
        let changed = match self.lookup {
            VariableLookup::Sheet => {
                let sheet = self.sheet?;
                let key = VariableKey::sheet(sheet, &self.name);
                host.changed_variables.contains(&key)
            }
            VariableLookup::Global => {
                let g = VariableKey::global(&self.name);
                let p = VariableKey::persistent(&self.name);
                host.changed_variables.contains(&g) || host.changed_variables.contains(&p)
            }
            VariableLookup::Auto => {
                if let Some(sheet) = self.sheet {
                    if host.active_sheet_variables.contains(&(sheet, self.name.clone())) {
                        let key = VariableKey::sheet(sheet, &self.name);
                        host.changed_variables.contains(&key)
                    } else {
                        let g = VariableKey::global(&self.name);
                        let p = VariableKey::persistent(&self.name);
                        host.changed_variables.contains(&g) || host.changed_variables.contains(&p)
                    }
                } else {
                    let g = VariableKey::global(&self.name);
                    let p = VariableKey::persistent(&self.name);
                    host.changed_variables.contains(&g) || host.changed_variables.contains(&p)
                }
            }
        };
        if changed {
            Some(PinId(0))
        } else {
            None
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let mut outputs = vec![NodeOutput::Execution(PinId(0))];
        if let Some(var) = ctx.variable(&self.name, self.lookup) {
            if let Some(val) = fit_variable_value_with_display(ctx.types, var.value.clone(), &self.value_type, &ctx.host_state.display, crate::quantity::labels(&ctx.host_state.locale)) {
                outputs.push(NodeOutput::Data(PinId(1), val));
            } else {
                outputs.push(NodeOutput::Clear(PinId(1)));
            }
        } else {
            outputs.push(NodeOutput::Clear(PinId(1)));
        }
        outputs
    }

    fn adapt_to(&mut self, pin: PinId, data_type: &DataType) -> bool {
        if pin != PinId(1) {
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
            _ => {}
        }
    }
}
