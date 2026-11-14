use crate::{Node, NodeContext};
use chrono::{Datelike, TimeZone, Timelike};
use ticknode_shared::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeriodUnit {
    Minute,
    Hour,
    #[default]
    Day,
    Week,
    Month,
    Year,
}

impl PeriodUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minute => "minute",
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "minute" => Some(Self::Minute),
            "hour" => Some(Self::Hour),
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            "year" => Some(Self::Year),
            _ => None,
        }
    }
}

pub struct StartOfPeriodNode {
    pub id: NodeId,
    pub unit: PeriodUnit,
}

impl StartOfPeriodNode {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            unit: PeriodUnit::Day,
        }
    }
}

impl Node for StartOfPeriodNode {
    fn id(&self) -> NodeId {
        self.id
    }

    fn is_pure(&self) -> bool {
        true
    }

    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            type_name: "time/start_of".into(),
            title: "Start of period".into(),
            category: "Time".into(),
            description: "Calculates the start of a period for a date-time".into(),
            help: "Calculates the start of the selected time period (minute, hour, day, week, month, or year) in the server local time zone, returning the result in UTC.\n\nWeek starts on Monday at 00:00:00 local time. Day starts at 00:00:00, Month on the 1st day at 00:00:00, and Year on January 1st at 00:00:00. For ambiguous or nonexistent local times due to daylight saving time transitions, the earliest valid instant is chosen, stepping forward minute by minute if needed.\n\nPure node: evaluated whenever its output is pulled by another node.".into(),
            cost: 1,
            properties: vec![
                PropertyDefinition::select(
                    "unit",
                    "Unit",
                    &[
                        ("minute", "Minute"),
                        ("hour", "Hour"),
                        ("day", "Day"),
                        ("week", "Week"),
                        ("month", "Month"),
                        ("year", "Year"),
                    ],
                )
                .described("Time period to find the start of"),
            ],
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
                    name: "Result".into(),
                    pin_type: PinType::Data(DataType::DateTime),
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
        let target_naive = match self.unit {
            PeriodUnit::Minute => local
                .date_naive()
                .and_hms_opt(local.hour(), local.minute(), 0),
            PeriodUnit::Hour => local.date_naive().and_hms_opt(local.hour(), 0, 0),
            PeriodUnit::Day => local.date_naive().and_hms_opt(0, 0, 0),
            PeriodUnit::Week => {
                let days_from_monday = local.weekday().num_days_from_monday();
                (local.date_naive() - chrono::TimeDelta::days(days_from_monday as i64))
                    .and_hms_opt(0, 0, 0)
            }
            PeriodUnit::Month => chrono::NaiveDate::from_ymd_opt(local.year(), local.month(), 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0)),
            PeriodUnit::Year => chrono::NaiveDate::from_ymd_opt(local.year(), 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0)),
        };

        let Some(mut current_naive) = target_naive else {
            return vec![];
        };

        let mut resolved = chrono::Local.from_local_datetime(&current_naive).earliest();
        if resolved.is_none() {
            for _ in 1..=180 {
                if let Some(next) = current_naive.checked_add_signed(chrono::TimeDelta::minutes(1))
                {
                    current_naive = next;
                    if let Some(r) = chrono::Local.from_local_datetime(&current_naive).earliest() {
                        resolved = Some(r);
                        break;
                    }
                } else {
                    break;
                }
            }
        }

        let Some(local_dt) = resolved else {
            return vec![];
        };

        vec![NodeOutput::Data(
            PinId(1),
            DataValue::DateTime(local_dt.with_timezone(&chrono::Utc)),
        )]
    }

    fn properties(&self) -> Vec<(String, DataValue)> {
        vec![("unit".into(), self.unit.as_str().into())]
    }

    fn set_property(&mut self, name: &str, value: DataValue) {
        if name == "unit"
            && let DataValue::String(s) = value
            && let Some(u) = PeriodUnit::from_str(&s)
        {
            self.unit = u;
        }
    }
}
