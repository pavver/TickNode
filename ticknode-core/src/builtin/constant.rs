use crate::{Node, NodeContext};
use ticknode_shared::*;

pub struct ConstNode {
    pub id: NodeId,
    pub value: DataValue,
    pub value_type: Option<DataType>,
}
impl Node for ConstNode {
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
            properties: Vec::new(),
            type_name: "const".to_string(),
            title: "Constant".to_string(),
            category: "Data".to_string(),
            description: "Returns a constant value".to_string(),
            help: "Emits the value set directly on the node (\"Value\" row) or in the right panel on output Val. The value type (number, string, boolean, etc.) is chosen on the node, and the pin type follows it.\n\nPure node: has no exec pins and is evaluated whenever its output is needed by another node.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Val".to_string(),
                pin_type: PinType::Data(self.value_type.clone().unwrap_or_else(|| self.value.get_type())),
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Data(PinId(0), self.value.clone())]
    }
    fn adapt_to(&mut self, pin: PinId, data_type: &DataType) -> bool {
        if pin != PinId(0) {
            return false;
        }
        let value = text::default_of(data_type);
        let declared = quantity::kind(&data_type_name(data_type)).map(|_| data_type.clone());
        if value == self.value && self.value_type == declared {
            return false;
        }
        self.value_type = declared;
        self.set_property("value", value);
        true
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        let mut props = Vec::new();
        if let Some(t) = &self.value_type {
            props.push(("value_type".into(), data_type_name(t).into()));
        }
        props.push(("value".into(), self.value.clone()));
        props
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value_type"
            && let DataValue::String(s) = &value
        {
            self.value_type = quantity::kind(s).map(|_| DataType::Named(s.clone()));
            if let Some(q) = quantity::kind(s)
                && !matches!(self.value, DataValue::Number(_))
            {
                self.value = match &self.value {
                    DataValue::String(text) => {
                        quantity::parse_quantity(text, q).unwrap_or(0.0).into()
                    }
                    _ => 0i64.into(),
                };
            }
        }
        if name == "value" {
            if !matches!(value, DataValue::Number(_)) {
                self.value_type = None;
            }
            self.value = value;
        }
    }
}
