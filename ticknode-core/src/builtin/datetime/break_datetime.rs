use crate::{Node, NodeContext};
use chrono::{Datelike, Timelike};
use ticknode_shared::*;

pub struct BreakDateTimeNode {
    pub id: NodeId,
}

impl BreakDateTimeNode {
    pub fn new(id: NodeId) -> Self {
        Self { id }
    }
}

impl Node for BreakDateTimeNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/break".into(),
            title: "Break Date-Time".into(),
            category: "Time".into(),
            description: "Extracts date and time components in the local time zone".into(),
            help: "Breaks a date-time into components in the server local time zone: Year, Month (1–12), Day (1–31), Weekday (1 = Monday … 7 = Sunday), Hour (0–23), Minute (0–59), Second (0–59), local Time of day, and Day of year (1–366).\n\nFor example, 2026-03-15 14:30:00 yields Year = 2026, Month = 3, Day = 15, Weekday = 7 (Sunday), Hour = 14, Minute = 30, Second = 0, Time of day = 14:30:00, and Day of year = 74.\n\nPure node: evaluated whenever any of its outputs are pulled by another node.".into(),
            cost: 1,
            pins: vec![
                PinDefinition {
                    id: PinId(0),
                    name: "Date-time".into(),
                    pin_type: PinType::Data(DataType::DateTime),
                    direction: PinDirection::Input,
                    required: true,
                },
                PinDefinition {
                    id: PinId(1),
                    name: "Year".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(2),
                    name: "Month".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(3),
                    name: "Day".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(4),
                    name: "Weekday".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(5),
                    name: "Hour".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(6),
                    name: "Minute".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(7),
                    name: "Second".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(8),
                    name: "Time of day".into(),
                    pin_type: PinType::Data(DataType::Time),
                    direction: PinDirection::Output,
                    required: false,
                },
                PinDefinition {
                    id: PinId(9),
                    name: "Day of year".into(),
                    pin_type: PinType::Data(DataType::Number),
                    direction: PinDirection::Output,
                    required: false,
                },
            ],
            ..Default::default()
        }
    }

    fn execute(&mut self, _: Option<PinId>, ctx: &mut NodeContext) -> Vec<NodeOutput> {
        let Some(DataValue::DateTime(dt)) = ctx.get_value(PinId(0)) else {
            return vec![];
        };
        let local = dt.with_timezone(&chrono::Local);
        vec![
            NodeOutput::Data(
                PinId(1),
                DataValue::Number(NumberValue::Int(local.year() as i64)),
            ),
            NodeOutput::Data(
                PinId(2),
                DataValue::Number(NumberValue::Int(local.month() as i64)),
            ),
            NodeOutput::Data(
                PinId(3),
                DataValue::Number(NumberValue::Int(local.day() as i64)),
            ),
            NodeOutput::Data(
                PinId(4),
                DataValue::Number(NumberValue::Int(local.weekday().number_from_monday() as i64)),
            ),
            NodeOutput::Data(
                PinId(5),
                DataValue::Number(NumberValue::Int(local.hour() as i64)),
            ),
            NodeOutput::Data(
                PinId(6),
                DataValue::Number(NumberValue::Int(local.minute() as i64)),
            ),
            NodeOutput::Data(
                PinId(7),
                DataValue::Number(NumberValue::Int(local.second() as i64)),
            ),
            NodeOutput::Data(PinId(8), DataValue::Time(local.time())),
            NodeOutput::Data(
                PinId(9),
                DataValue::Number(NumberValue::Int(local.ordinal() as i64)),
            ),
        ]
    }
}
