use crate::{Node, NodeContext};
use ticknode_shared::*;

pub struct NowNode {
    pub id: NodeId,
}

impl NowNode {
    pub fn new(id: NodeId) -> Self {
        Self { id }
    }
}

impl Node for NowNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/now".into(),
            title: "Now".into(),
            category: "Time".into(),
            description: "Current date-time and local time of day".into(),
            help: "Outputs the current date-time in UTC on Now and the current time of day in the server local time zone on Time of day.\n\nUseful as a time source for comparisons, conditions, and calculations.\n\nPure node: evaluated whenever its outputs are pulled by another node.".into(),
            cost: 1,
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Now".into(),
                    pin_type: PinType::Data(DataType::DateTime),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Time of day".into(),
                    pin_type: PinType::Data(DataType::Time),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let now = ctx.host_state.now();
        let local = now.with_timezone(&chrono::Local);
        vec![
            NodeOutput::Data(PinId(0), DataValue::DateTime(now)),
            NodeOutput::Data(PinId(1), DataValue::Time(local.time())),
        ]
    }
}
