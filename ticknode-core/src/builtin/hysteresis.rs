use ticknode_shared::*;
use crate::{Node, NodeContext};
use super::math::numeric_type;

pub struct HysteresisNode {
    pub id: NodeId,
    value_type: DataType,
    falling: bool,
    state: bool,
    invalid_reported: bool,
}

impl HysteresisNode {
    pub fn new(id: NodeId) -> Self {
        Self { id, value_type: DataType::Number, falling: false, state: false, invalid_reported: false }
    }
}

impl Node for HysteresisNode {
    fn id(&self) -> NodeId { self.id }

    fn definition(&self) -> NodeDefinition {
        let input = |id, name: &str| PinDefinition {
            id: PinId(id), name: name.into(), pin_type: PinType::Data(self.value_type.clone()),
            direction: PinDirection::Input, required: true,
        };
        let exec = |id, name: &str, direction| PinDefinition {
            id: PinId(id), name: name.into(), pin_type: PinType::Exec, direction, required: false,
        };
        NodeDefinition {
            type_name: "logic/hysteresis".into(),
            title: "Hysteresis".into(),
            category: "Logic".into(),
            description: "Switches a state using separate on and off thresholds".into(),
            help: "On In, evaluates Value against On threshold and Off threshold. Rising mode turns on strictly above On threshold and off strictly below Off threshold. For example, allow charging above 230 V and forbid it below 200 V. Falling mode reverses the comparisons: heating turns on below 19 °C and off above 21 °C.\n\nBetween thresholds, including equality, the previous state is kept. The initial state is off, so the first evaluation in the dead band yields off. State lives in memory and resets to off after a restart or node recreation.\n\nValue and thresholds share a Number or physical quantity type, adapted when Value is connected. Unconnected thresholds can be entered next to their pins with a quantity unit control. All three inputs are required.\n\nEvery evaluation emits State first, then Turned on or Turned off only if the state changed, then Out. Rising mode requires On threshold ≥ Off threshold; falling mode requires On threshold ≤ Off threshold. Invalid thresholds report an error and keep the state, but still emit State and Out.".into(),
            adaptive: true,
            cost: 1,
            properties: vec![PropertyDefinition::select("mode", "Mode", &[
                ("rising", "Rising"), ("falling", "Falling"),
            ]).described("Whether rising or falling values turn the state on")],
            pins: vec![
                exec(0, "In", PinDirection::Input),
                input(1, "Value"),
                input(2, "On threshold"),
                input(3, "Off threshold"),
                exec(4, "Turned on", PinDirection::Output),
                exec(5, "Turned off", PinDirection::Output),
                exec(6, "Out", PinDirection::Output),
                PinDefinition {
                    id: PinId(7), name: "State".into(), pin_type: PinType::Data(DataType::Bool),
                    direction: PinDirection::Output, required: false,
                },
            ],
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let value = ctx.get_number(PinId(1), NumberValue::Int(0)).to_f64();
        let on = ctx.get_number(PinId(2), NumberValue::Int(0)).to_f64();
        let off = ctx.get_number(PinId(3), NumberValue::Int(0)).to_f64();
        let previous = self.state;
        let invalid = if self.falling { on > off } else { on < off };
        if invalid {
            let message = UiMessage::new("error-hysteresis-thresholds", "Invalid hysteresis thresholds: rising mode requires On threshold ≥ Off threshold; falling mode requires On threshold ≤ Off threshold");
            if !self.invalid_reported {
                ctx.host_state.error(self.id, message.clone(), format!("[ERROR] {}", message.fallback));
                self.invalid_reported = true;
            } else {
                // Keep the UI error alive while Out continues, without repeating the log.
                ctx.host_state.report_error(self.id, message);
            }
        } else {
            self.invalid_reported = false;
            if if self.falling { value < on } else { value > on } {
                self.state = true;
            } else if if self.falling { value > off } else { value < off } {
                self.state = false;
            }
        }
        let mut outputs = vec![NodeOutput::Data(PinId(7), DataValue::Bool(self.state))];
        if self.state != previous {
            outputs.push(NodeOutput::Execution(PinId(if self.state { 4 } else { 5 })));
        }
        outputs.push(NodeOutput::Execution(PinId(6)));
        outputs
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![
            ("value_type".into(), data_type_name(&self.value_type).into()),
            ("mode".into(), if self.falling { "falling" } else { "rising" }.into()),
        ]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        let DataValue::String(value) = value else { return };
        match name {
            "value_type" => {
                if let Some(t) = parse_data_type(&value).filter(numeric_type) { self.value_type = t; }
            }
            "mode" if value == "rising" || value == "falling" => self.falling = value == "falling",
            _ => {}
        }
    }

    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        if !(1..=3).contains(&pin.0) || !numeric_type(t) || *t == self.value_type { return false; }
        self.value_type = t.clone();
        true
    }

    fn adapts_on_connect(&self, pin: PinId) -> bool { pin == PinId(1) }
}
