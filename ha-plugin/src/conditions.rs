//! HA-style gates. All evaluations except Jinja templates use the shared live cache.

mod evaluation;
mod node;

pub use evaluation::{evaluate, template_bool};
pub use node::ConditionNode;

use crate::CATEGORY;
use ticknode_plugin_sdk::*;

pub const YES: PinId = PinId(1);
pub const NO: PinId = PinId(2);
pub const RESULT: PinId = PinId(3);

pub fn outputs(value: bool) -> Vec<Output> {
    vec![
        Output::Data(RESULT, DataValue::Bool(value)),
        Output::Exec(if value { YES } else { NO }),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Time,
    Sun,
    Zone,
    Template,
}
pub const KINDS: &[Kind] = &[Kind::Time, Kind::Sun, Kind::Zone, Kind::Template];
impl Kind {
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Time => "ha/condition_time",
            Self::Sun => "ha/condition_sun",
            Self::Zone => "ha/condition_zone",
            Self::Template => "ha/condition_template",
        }
    }
    pub fn definition(self) -> NodeDefinition {
        let sun = |name, title| {
            PropertyDefinition::select(
                name,
                title,
                &[
                    ("none", "None"),
                    ("sunrise", "Sunrise"),
                    ("sunset", "Sunset"),
                ],
            )
            .described("Optional solar event boundary")
        };
        let (title, description, help, properties) = match self {
            Self::Time => (
                "Time condition",
                "Checks local time and weekdays in Home Assistant",
                "After is inclusive; Before is exclusive. Equal After/Before times cover the full day. A window crossing midnight uses either side of midnight; weekdays always refer to the current local day. Choose weekdays with the day buttons; stored mon,tue,wed,thu,fri,sat,sun values remain supported. Empty fields impose no restriction. Uses get_config time_zone and a live HA-rendered UTC offset, never the WASI host timezone. HA refreshes the offset every minute: a timezone/DST change can lag by up to one minute plus event delivery. Until timezone data arrives the result is No.",
                vec![
                    PropertyDefinition::time("after", "After")
                        .described("Optional local HH:MM[:SS], inclusive"),
                    PropertyDefinition::time("before", "Before")
                        .described("Optional local HH:MM[:SS], exclusive"),
                    PropertyDefinition::multi_select(
                        "weekday",
                        "Weekdays",
                        &[
                            ("mon", "Monday"),
                            ("tue", "Tuesday"),
                            ("wed", "Wednesday"),
                            ("thu", "Thursday"),
                            ("fri", "Friday"),
                            ("sat", "Saturday"),
                            ("sun", "Sunday"),
                        ],
                    )
                    .described("Optional weekdays; empty means every day"),
                ],
            ),
            Self::Sun => (
                "Sun condition",
                "Checks sunrise and sunset boundaries with offsets",
                "Uses cached sun.sun next_rising/next_setting. Each next event's HA-local wall time is projected onto today's local date, then the signed offset is added. Before and After are inclusive, independent tests combined with AND, as in HA (After sunset plus Before sunrise does not form an overnight window). Projection is an approximation after an event has passed: tomorrow's solar time may differ by a few minutes. Polar events more than two days away fail to No. Offsets accept -30m or 00:30:00; -30m means thirty minutes earlier. An offset appears only when its boundary is enabled. Uses the same HA UTC offset and minute refresh limitation as Time.",
                vec![
                    sun("after", "After"),
                    PropertyDefinition::duration("after_offset", "Offset", true)
                        .described("Optional signed duration; -30m means thirty minutes earlier"),
                    sun("before", "Before"),
                    PropertyDefinition::duration("before_offset", "Offset", true)
                        .described("Optional signed duration; -30m means thirty minutes earlier"),
                ],
            ),
            Self::Zone => (
                "Zone condition",
                "Checks whether a person or tracker is in a zone",
                "Compares the cached person/device_tracker state with the zone's friendly_name; zone.home matches home. Pick a person or type a device_tracker entity id. This checks HA's reported zone name, without a separate GPS/radius calculation; overlapping zones follow the entity's reported state.",
                vec![
                    PropertyDefinition::dynamic("entity_id", "Entity", "persons")
                        .described("Person or device_tracker entity id"),
                    PropertyDefinition::dynamic("zone", "Zone", "zones")
                        .described("Zone to match; zone.home matches home"),
                ],
            ),
            Self::Template => (
                "Template condition",
                "Evaluates a Jinja condition asynchronously in Home Assistant",
                "On In starts a render_template subscription in Home Assistant and uses its first result, then unsubscribes. Result and Yes/No arrive a few ticks later. Only boolean true or text true (case-insensitive) is true, matching HA template conditions; other values are false. A render error, disconnected connection or 30-second timeout yields No and an English warning. Each In has its own request (up to 64 in flight per plugin); changing properties or deleting the node cancels its pending requests.",
                vec![
                    PropertyDefinition::multiline("value_template", "Value template")
                        .described("Home Assistant Jinja template whose result is boolean"),
                ],
            ),
        };
        NodeDefinition {
            adaptive: false,
            cost: 2,
            type_name: self.type_name().into(),
            title: title.into(),
            description: description.into(),
            help: format!(
                "{help}\n\nIn evaluates the condition. Result is boolean; exactly one of Yes or No fires. Local conditions make no request on execution. Invalid stored settings show a node error and log once until fixed. Unknown, unavailable, not yet loaded or disconnected states yield No and a warning once until that situation changes."
            ),
            category: CATEGORY.into(),
            properties,
            pins: vec![
                exec_in(0, "In"),
                exec_out(YES.0, "Yes"),
                exec_out(NO.0, "No"),
                data_out(RESULT.0, "Result", DataType::Bool),
            ],
        }
    }
}

#[cfg(test)]
mod tests;
