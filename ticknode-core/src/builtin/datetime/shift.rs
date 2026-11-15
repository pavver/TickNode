use crate::{Node, NodeContext};
use ticknode_shared::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShiftOperation {
    #[default]
    Add,
    Subtract,
}

impl ShiftOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Subtract => "subtract",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "add" => Some(Self::Add),
            "subtract" => Some(Self::Subtract),
            _ => None,
        }
    }
}

pub struct ShiftTimeNode {
    pub id: NodeId,
    pub value_type: DataType,
    pub operation: ShiftOperation,
    pub overflow_reported: bool,
}

impl ShiftTimeNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            value_type: DataType::DateTime,
            operation: ShiftOperation::Add,
            overflow_reported: false,
        }
    }
}

impl Node for ShiftTimeNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/shift".into(),
            title: "Shift Time".into(),
            category: "Time".into(),
            description: "Shifts a date-time or time of day by a duration".into(),
            help: "Shifts Value forward or backward by Duration: Result = Value + Duration (or − Duration).\n\nWorks with both date-times and times of day. Connecting a wire to Value adapts the input and output to that type. Shifting a time of day wraps around midnight, so 23:00 + 2 hours yields 01:00. Date-time arithmetic checks for overflow and reports an error if the result exceeds valid date boundaries.\n\nPure node: evaluated whenever its output is pulled by another node.".into(),
            adaptive: true,
            cost: 1,
            properties: vec![
                PropertyDefinition::select(
                    "operation",
                    "Operation",
                    &[("add", "Add"), ("subtract", "Subtract")],
                )
                .described("Whether to add or subtract duration"),
            ],
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Value".into(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Duration".into(),
                    pin_type: PinType::Data(DataType::Duration),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Result".into(),
                    pin_type: PinType::Data(self.value_type.clone()),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let Some(val) = ctx.get_value(PinId(0)) else {
            return vec![];
        };
        let Some(dur_val) = ctx.get_value(PinId(1)) else {
            return vec![];
        };
        let DataValue::Duration(duration) = dur_val else {
            return vec![];
        };

        let delta = match self.operation {
            ShiftOperation::Add => duration,
            ShiftOperation::Subtract => -duration,
        };

        match val {
            DataValue::Time(t) => {
                let (new_time, _days) = t.overflowing_add_signed(delta);
                self.overflow_reported = false;
                vec![NodeOutput::Data(PinId(2), DataValue::Time(new_time))]
            }
            DataValue::DateTime(dt) => {
                if let Some(res) = dt.checked_add_signed(delta) {
                    self.overflow_reported = false;
                    vec![NodeOutput::Data(PinId(2), DataValue::DateTime(res))]
                } else {
                    let message = UiMessage::new(
                        "error-time-shift-overflow",
                        "Date-time shift arithmetic overflowed",
                    );
                    if !self.overflow_reported {
                        ctx.host_state.error(
                            self.id,
                            message.clone(),
                            format!("[ERROR] {}", message.fallback),
                        );
                        self.overflow_reported = true;
                    } else {
                        ctx.host_state.report_error(self.id, message);
                    }
                    vec![]
                }
            }
            _ => vec![],
        }
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        let mut props = Vec::new();
        if self.value_type != DataType::DateTime {
            props.push(("value_type".into(), data_type_name(&self.value_type).into()));
        }
        props.push(("operation".into(), self.operation.as_str().into()));
        props
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        match name {
            "value_type" => {
                if let DataValue::String(s) = value
                    && let Some(t) = parse_data_type(&s)
                    && (t == DataType::DateTime || t == DataType::Time)
                {
                    self.value_type = t;
                }
            }
            "operation" => {
                if let DataValue::String(s) = value
                    && let Some(op) = ShiftOperation::from_str(&s)
                {
                    self.operation = op;
                }
            }
            _ => {}
        }
    }

    fn adapt_to(&mut self, pin: PinId, t: &DataType) -> bool {
        let adapts = pin == PinId(0) || pin == PinId(2);
        if !adapts || (*t != DataType::DateTime && *t != DataType::Time) || *t == self.value_type {
            return false;
        }
        self.value_type = t.clone();
        true
    }

    fn adapts_on_connect(&self, pin: PinId) -> bool {
        pin == PinId(0)
    }
}
