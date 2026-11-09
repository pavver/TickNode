use crate::{HostState, TaskId, TaskResult};
use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ClockNode { pub id: NodeId, pub interval_ticks: u64 }
impl Node for ClockNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "clock".to_string(),
            title: "Clock".to_string(),
            category: "Events".to_string(),
            description: "Emits an execution pulse every interval_ticks ticks".to_string(),
            help: "Triggers execution on its own: every N ticks (\"Interval\" property) it emits a signal on Out.\n\nThe server tick is 100 ms, so 10 ticks ≈ 1 second. This is the primary event source for chains that run continuously.".to_string(),
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Out".to_string(),
                pin_type: PinType::Exec,
                direction: PinDirection::Output, required: false }],
        }
    }
    fn execute(&mut self, _: Option<PinId>, _: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(0))]
    }
    fn polls(&self) -> bool { true }
    fn poll(&mut self, host: &mut HostState) -> Option<PinId> {
        (host.current_tick % self.interval_ticks.max(1) == 0).then_some(PinId(0))
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "interval_ticks" {
            if let DataValue::Number(n) = value {
                self.interval_ticks = n.to_f64().max(1.0) as u64;
            }
        }
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("interval_ticks".to_string(), DataValue::Number(NumberValue::Int(self.interval_ticks as i64)))]
    }
}

pub struct SleepNode { pub id: NodeId }
impl Node for SleepNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            adaptive: false,
            cost: 1,
            properties: Vec::new(),
            type_name: "sleep".to_string(),
            title: "Sleep".to_string(),
            category: "Time".to_string(),
            description: "Delays execution by a duration in ticks or time".to_string(),
            help: "A signal at In starts a delay, and when it elapses, emits a signal on Out.\n\n• Delay — duration in ticks or time, e.g. 10t or 5s. Time is rounded up to whole server ticks. Without a value, waits one tick.\n\nEach signal at In starts its own independent delay, like Queue Delay.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Delay".to_string(), pin_type: PinType::Data(DataType::Duration), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let ticks = ctx.get_value(PinId(1))
                .as_ref().and_then(|value| ctx.host_state.ticks_for_value(value)).unwrap_or(1);
            ctx.host_state.start_delay(self.id, ticks);
        }
        vec![]
    }
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(4))]
    }
}
