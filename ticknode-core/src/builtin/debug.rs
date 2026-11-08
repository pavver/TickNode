use tracing::info;
use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ConsoleNode { pub id: NodeId }
impl Node for ConsoleNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            properties: Vec::new(),
            type_name: "console".to_string(),
            title: "Console Log".to_string(),
            category: "Debug".to_string(),
            description: "Prints a value to the console when triggered".to_string(),
            help: "On a signal at In, prints the value of Val to the editor console and server log. Without Val connected, prints \"None\".\n\nHas no output — this is a terminal node in a chain. To log a value in the middle of a chain, use Debug Log.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let message = val.as_ref().map(format_data_value).unwrap_or_else(|| "None".to_string());
        info!(%message, "Console Output");
        ctx.host_state.logs.push((self.id, message));
        vec![]
    }
}

pub fn format_data_value(val: &DataValue) -> String {
    match val {
        DataValue::Bool(b) => b.to_string(),
        DataValue::Number(num) => match num {
            NumberValue::Int(i) => i.to_string(),
            NumberValue::Float(f) => f.to_string(),
        },
        DataValue::String(s) => s.clone(),
        DataValue::Time(t) => t.to_string(),
        DataValue::DateTime(dt) => dt.to_string(),
        DataValue::Duration(d) => format!("{:?}", d),
        DataValue::Color(c) => format!("Color(r:{}, g:{}, b:{}, a:{})", c.r, c.g, c.b, c.a),
        DataValue::List { elements, .. } => {
            let items: Vec<String> = elements.iter().map(format_data_value).collect();
            format!("[{}]", items.join(", "))
        }
        DataValue::Object { fields, .. } => {
            let mut items: Vec<String> = fields.iter()
                .map(|(k, v)| format!("{}: {}", k, format_data_value(&v.value)))
                .collect();
            items.sort();
            format!("{{{}}}", items.join(", "))
        }
    }
}

pub struct DebugNode {
    pub id: NodeId,
    pub prefix: String,
}

impl Node for DebugNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            properties: Vec::new(),
            type_name: "test/debug".to_string(),
            title: "Debug Log".to_string(),
            category: "Debug".to_string(),
            description: "Accepts any data and logs it with a text prefix".to_string(),
            help: "On a signal at In, logs the \"Prefix\" together with the value of Val to the console and passes execution to Out.\n\nCan be inserted in the middle of a chain to inspect the value passing through it.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Val".to_string(), pin_type: PinType::Data(DataType::Any), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let val = ctx.get_value(PinId(1));
        let formatted = match val {
            Some(v) => format_data_value(&v),
            None => "None".to_string(),
        };
        let message = format!("{}{}", self.prefix, formatted);
        info!("{}", message);
        ctx.host_state.logs.push((self.id, message));
        vec![NodeOutput::Execution(PinId(2))]
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "prefix" {
            if let DataValue::String(s) = value {
                self.prefix = s;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("prefix".to_string(), DataValue::String(self.prefix.clone()))]
    }
}
