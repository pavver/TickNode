use crate::{HostState, TaskId, TaskResult};
use ticknode_shared::*;
use crate::{Node, NodeContext};

pub struct ClockNode { pub id: NodeId, pub interval_ticks: u64 }
impl Node for ClockNode {
    fn id(&self) -> NodeId { self.id }
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
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
            cost: 1,
            properties: Vec::new(),
            type_name: "sleep".to_string(),
            title: "Sleep".to_string(),
            category: "Time".to_string(),
            description: "Delays execution by the specified number of milliseconds or ticks".to_string(),
            help: "A signal at In starts a delay, and when it elapses, emits a signal on Out.\n\n• Delay — duration in milliseconds (rounded up to whole ticks).\n• UseTicks = true — Delay is counted in ticks.\n• TickDuration — tick duration in ms for calculation (defaults to the server tick).\n\nEach signal at In starts its own independent delay, like Queue Delay.".to_string(),
            pins: vec![
                PinDefinition { id: PinId(0), name: "In".to_string(), pin_type: PinType::Exec, direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(1), name: "Delay".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(2), name: "UseTicks".to_string(), pin_type: PinType::Data(DataType::Bool), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(3), name: "TickDuration".to_string(), pin_type: PinType::Data(DataType::Number), direction: PinDirection::Input, required: false },
                PinDefinition { id: PinId(4), name: "Out".to_string(), pin_type: PinType::Exec, direction: PinDirection::Output, required: false },
            ],
        }
    }
    fn execute(&mut self, trigger_pin: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        if trigger_pin == Some(PinId(0)) {
            let delay = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64().max(0.0);
            let ticks = if ctx.get_bool(PinId(2), false) {
                delay.ceil() as u64
            } else {
                // Milliseconds; TickDuration (if set) overrides the engine's tick duration.
                match ctx.get_number(PinId(3), NumberValue::Int(0)).to_f64() {
                    tick_ms if tick_ms > 0.0 => (delay / tick_ms).ceil() as u64,
                    _ => ctx.host_state.ticks_for(std::time::Duration::from_millis(delay as u64)),
                }
            };
            ctx.host_state.start_delay(self.id, ticks);
        }
        vec![]
    }
    fn on_task(&mut self, _task: TaskId, _result: TaskResult, _ctx: &mut NodeContext) -> Vec<NodeOutput> {
        vec![NodeOutput::Execution(PinId(4))]
    }
}
