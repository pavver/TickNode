use crate::{Node, NodeContext};
use ticknode_shared::*;

pub struct TimeDifferenceNode {
    pub id: NodeId,
    pub value_type: DataType,
}

impl TimeDifferenceNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            value_type: DataType::DateTime,
        }
    }
}

impl Node for TimeDifferenceNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/difference".into(),
            title: "Time difference".into(),
            category: "Time".into(),
            description: "Duration between two date-times or times of day".into(),
            help: "Computes the difference between two moments: Result = A − B.\n\nWorks with both date-times and times of day. Connecting a wire to A or B adapts the inputs to that type. When comparing times of day, the calculation is a plain difference without wrapping around midnight, so 10:00 − 14:00 yields −4 hours.\n\nPure node: evaluated whenever its output is pulled by another node.".into(),
            adaptive: true,
            cost: 1,
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "A".into(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "B".into(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Result".into(),
                    pin_type: PinType::Data(DataType::Duration),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let a_val = ctx.get_value(PinId(0));
        let b_val = ctx.get_value(PinId(1));
        let delta = match (&a_val, &b_val) {
            (Some(DataValue::DateTime(a)), Some(DataValue::DateTime(b))) => {
                a.signed_duration_since(*b)
            }
            (Some(DataValue::Time(a)), Some(DataValue::Time(b))) => a.signed_duration_since(*b),
            _ => return vec![],
        };
        vec![NodeOutput::Data(PinId(2), DataValue::Duration(delta))]
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        if self.value_type == DataType::DateTime {
            Vec::new()
        } else {
            vec![("value_type".into(), data_type_name(&self.value_type).into())]
        }
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "value_type"
            && let DataValue::String(s) = value
            && let Some(t) = parse_data_type(&s)
            && (t == DataType::DateTime || t == DataType::Time)
        {
            self.value_type = t;
        }
    }

    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        let adapts = pin == PinId(0) || pin == PinId(1);
        if !adapts || (*t != DataType::DateTime && *t != DataType::Time) || *t == self.value_type {
            return false;
        }
        self.value_type = t.clone();
        true
    }

    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(0) || pin == PinId(1)
    }
}
