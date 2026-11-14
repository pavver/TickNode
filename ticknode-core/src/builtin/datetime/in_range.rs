use crate::{Node, NodeContext};
use ticknode_shared::*;

pub struct TimeInRangeNode {
    pub id: NodeId,
}

impl TimeInRangeNode {
    pub fn new(id: NodeId) -> Self {
        Self { id }
    }
}

impl Node for TimeInRangeNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/in_range".into(),
            title: "Time of day in range".into(),
            category: "Time".into(),
            description: "Checks whether a time of day falls between From and To".into(),
            help: "Checks whether the local time of day falls within the range from From to To: From ≤ t < To.\n\nWhen From > To, the range crosses midnight (t ≥ From or t < To), for example 22:00 to 06:00 matches night hours. When From == To, the range is empty and always yields false. If Value is unconnected, the current time from the engine clock is used.\n\nPure node: evaluated whenever its output is pulled by another node.".into(),
            cost: 1,
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Value".into(),
                    pin_type: PinType::Data(DataType::DateTime),
                    direction: PinDirection::Input,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "From".into(),
                    pin_type: PinType::Data(DataType::Time),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "To".into(),
                    pin_type: PinType::Data(DataType::Time),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(3),
                    name: "In range".into(),
                    pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let t = match ctx.get_value(PinId(0)) {
            Some(DataValue::DateTime(dt)) => dt.with_timezone(&chrono::Local).time(),
            _ => ctx.host_state.now().with_timezone(&chrono::Local).time(),
        };
        let Some(DataValue::Time(from)) = ctx.get_value(PinId(1)) else {
            return vec![];
        };
        let Some(DataValue::Time(to)) = ctx.get_value(PinId(2)) else {
            return vec![];
        };

        let in_range = if from == to {
            false
        } else if from < to {
            from <= t && t < to
        } else {
            t >= from || t < to
        };

        vec![NodeOutput::Data(PinId(3), DataValue::Bool(in_range))]
    }
}
