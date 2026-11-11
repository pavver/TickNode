//! Automation trigger definitions, typed configuration and trigger-variable outputs.

mod config;
mod node;
mod outputs;

pub use config::{TriggerConfig, build, duration};
pub use node::TriggerNode;
pub use outputs::outputs;

use crate::CATEGORY;
use crate::convert::STATE_TYPE;
use std::collections::BTreeMap;
use ticknode_plugin_sdk::*;

pub const TRIGGERED: PinId = PinId(0);
pub const INFO: PinId = PinId(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Time,
    TimePattern,
    Sun,
    Zone,
    Event,
    HomeAssistant,
    Mqtt,
    Webhook,
    Calendar,
    Tag,
    Conversation,
    Template,
}

pub const PLATFORMS: &[Platform] = &[
    Platform::Time,
    Platform::TimePattern,
    Platform::Sun,
    Platform::Zone,
    Platform::Event,
    Platform::HomeAssistant,
    Platform::Mqtt,
    Platform::Webhook,
    Platform::Calendar,
    Platform::Tag,
    Platform::Conversation,
    Platform::Template,
];

impl Platform {
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Time => "ha/trigger_time",
            Self::TimePattern => "ha/trigger_time_pattern",
            Self::Sun => "ha/trigger_sun",
            Self::Zone => "ha/trigger_zone",
            Self::Event => "ha/trigger_event",
            Self::HomeAssistant => "ha/trigger_homeassistant",
            Self::Mqtt => "ha/trigger_mqtt",
            Self::Webhook => "ha/trigger_webhook",
            Self::Calendar => "ha/trigger_calendar",
            Self::Tag => "ha/trigger_tag",
            Self::Conversation => "ha/trigger_conversation",
            Self::Template => "ha/trigger_template",
        }
    }

    fn texts(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Time => (
                "Time trigger",
                "Fires at a time or an entity-provided timestamp",
                "At accepts HH:MM[:SS], an input_datetime entity or a timestamp sensor entity. Home Assistant schedules the trigger in its own time zone. A fixed time fires daily.",
            ),
            Self::TimePattern => (
                "Time pattern trigger",
                "Fires when the clock matches a repeating pattern",
                "Set Hours (0–23), Minutes and Seconds (0–59). Use /5 for multiples of five or * for any value. Leave unused fields empty; at least one field must be set. Scheduling and defaults of omitted fields are handled by Home Assistant.",
            ),
            Self::Sun => (
                "Sun trigger",
                "Fires at sunrise or sunset with an optional offset",
                "Choose Sunrise or Sunset. Offset accepts durations such as 30m or 00:30:00; a negative offset such as -00:30:00 fires before the event. Home Assistant uses its configured location.",
            ),
            Self::Zone => (
                "Zone trigger",
                "Fires when a person or device enters or leaves a zone",
                "Choose a person or device tracker, a zone, and Enter or Leave. Home Assistant detects the zone crossing. Entity, From and To describe the tracked entity; an absent state clears the corresponding output.",
            ),
            Self::Event => (
                "Event trigger",
                "Fires when a Home Assistant bus event matches",
                "Event type is the bus event name. Optional Event data is a JSON object whose fields filter the event. Event data must be a JSON object; invalid settings show a node error and prevent subscribing. Event data and Event type outputs describe the received event.",
            ),
            Self::HomeAssistant => (
                "Home Assistant start/stop trigger",
                "Fires when Home Assistant starts or shuts down",
                "Choose Start or Shutdown. The WebSocket must be connected when Home Assistant fires the event; startup events before the connection exists are not replayed. A shutdown pulse may arrive shortly before the connection closes.",
            ),
            Self::Mqtt => (
                "MQTT trigger",
                "Fires when an MQTT message matches a topic",
                "Topic accepts MQTT wildcards. Optional Payload filters by exact payload text. Choose QoS 0, 1 or 2. Home Assistant needs the MQTT integration. Topic and Payload outputs contain the received message.",
            ),
            Self::Webhook => (
                "Webhook trigger",
                "Fires when a Home Assistant webhook receives a request",
                "Set a unique Webhook ID. Send POST or PUT requests to Home Assistant's /api/webhook/<id> endpoint. Local only defaults to Yes. Data contains the JSON body or form data; Query contains URL query parameters. A non-object JSON body is wrapped in a value field. Treat the webhook id as a credential.",
            ),
            Self::Calendar => (
                "Calendar trigger",
                "Fires when a calendar event starts or ends",
                "Choose a calendar entity and Start or End. Optional Offset accepts signed durations; -15m fires fifteen minutes before the event. Event contains the calendar_event trigger variable, including the event summary and times.",
            ),
            Self::Tag => (
                "Tag trigger",
                "Fires when a Home Assistant tag is scanned",
                "Set the Tag ID from Home Assistant. Tag id and Device id outputs identify the scanned tag and scanning device. No device filter is applied.",
            ),
            Self::Conversation => (
                "Sentence trigger",
                "Fires when Assist matches a sentence",
                "Enter one or more sentence patterns separated by semicolons or newlines. Home Assistant's sentence syntax supports optional words and slots. Sentence contains the spoken text and Slots the matched values. Requires a conversation integration.",
            ),
            Self::Template => (
                "Template trigger",
                "Fires when a Home Assistant template becomes true",
                "Value template is a Home Assistant Jinja template. It fires when the result changes from false to true. Optional For requires it to remain true for a duration. Template evaluation and entity tracking run in Home Assistant.",
            ),
        }
    }

    pub fn properties(self) -> Vec<PropertyDefinition> {
        let text =
            |name, title, description| PropertyDefinition::text(name, title).described(description);
        let entity = |source| {
            PropertyDefinition::dynamic("entity_id", "Entity", source)
                .described("Choose an entity or type its entity id")
        };
        let duration = || PropertyDefinition::duration("for", "For", false).described("Optional non-negative duration: 90, 1h 30m or 00:05:00");
        let offset = || {
            PropertyDefinition::duration("offset", "Offset", true)
                .described("Optional signed duration; -30m means thirty minutes earlier")
        };
        let select = |name, title, options: &[(&str, &str)], description| {
            PropertyDefinition::select(name, title, options).described(description)
        };
        match self {
            Self::Time => vec![PropertyDefinition::dynamic(
                "at",
                "At",
                "time_entities",
            ).described("HH:MM[:SS], an input_datetime entity or a timestamp sensor entity")],
            Self::TimePattern => vec![
                text("hours", "Hours", "0–23, * or a step such as /2"),
                text("minutes", "Minutes", "0–59, * or a step such as /5"),
                text("seconds", "Seconds", "0–59, * or a step such as /10"),
            ],
            Self::Sun => {
                vec![
                    select(
                        "event",
                        "Event",
                        &[("sunrise", "Sunrise"), ("sunset", "Sunset")],
                        "The sun event to wait for",
                    ),
                    offset(),
                ]
            }
            Self::Zone => vec![
                entity("persons"),
                PropertyDefinition::dynamic("zone", "Zone", "zones")
                    .described("The zone to enter or leave"),
                select(
                    "event",
                    "Event",
                    &[("enter", "Enter"), ("leave", "Leave")],
                    "The zone crossing to wait for",
                ),
            ],
            Self::Event => vec![
                text("event_type", "Event type", "Home Assistant bus event name"),
                PropertyDefinition::multiline(
                    "event_data",
                    "Event data",
                ).described("Optional JSON object used to filter event fields"),
            ],
            Self::HomeAssistant => {
                vec![select(
                    "event",
                    "Event",
                    &[("start", "Start"), ("shutdown", "Shutdown")],
                    "The Home Assistant lifecycle event",
                )]
            }
            Self::Mqtt => vec![
                text("topic", "Topic", "MQTT topic, including optional wildcards"),
                text("payload", "Payload", "Optional exact payload filter"),
                select(
                    "qos",
                    "QoS",
                    &[("0", "0"), ("1", "1"), ("2", "2")],
                    "MQTT quality of service",
                ),
            ],
            Self::Webhook => vec![
                text(
                    "webhook_id",
                    "Webhook ID",
                    "Unique identifier for /api/webhook/<id>",
                ),
                select(
                    "local_only",
                    "Local only",
                    &[("yes", "Yes"), ("no", "No")],
                    "Only accept requests from the local network",
                ),
            ],
            Self::Calendar => vec![
                entity("calendars"),
                select(
                    "event",
                    "Event",
                    &[("start", "Start"), ("end", "End")],
                    "The calendar event boundary",
                ),
                offset(),
            ],
            Self::Tag => vec![text(
                "tag_id",
                "Tag ID",
                "The Home Assistant tag identifier",
            )],
            Self::Conversation => vec![PropertyDefinition::multiline(
                "command",
                "Sentence",
            ).described("Sentence patterns separated by semicolons or newlines")],
            Self::Template => vec![
                PropertyDefinition::multiline(
                    "value_template",
                    "Value template",
                ).described("Home Assistant Jinja template that becomes true"),
                duration(),
            ],
        }
    }

    pub fn defaults(self) -> BTreeMap<String, String> {
        self.properties()
            .into_iter()
            .map(|p| {
                let value = match &p.kind {
                    PropertyKind::Select(options) => {
                        options.first().map(|(v, _)| v.clone()).unwrap_or_default()
                    }
                    _ => String::new(),
                };
                (p.name, value)
            })
            .collect()
    }

    pub fn definition(self) -> NodeDefinition {
        let (title, description, help) = self.texts();
        let mut pins = vec![
            exec_out(0, "Triggered"),
            data_out(1, "Info", DataType::Object),
        ];
        let extra: Vec<(&str, DataType)> = match self {
            Self::Zone => vec![
                ("Entity", DataType::String),
                ("From", DataType::Named(STATE_TYPE.into())),
                ("To", DataType::Named(STATE_TYPE.into())),
            ],
            Self::Event => vec![
                ("Event data", DataType::Object),
                ("Event type", DataType::String),
            ],
            Self::Mqtt => vec![("Topic", DataType::String), ("Payload", DataType::String)],
            Self::Webhook => vec![("Data", DataType::Object), ("Query", DataType::Object)],
            Self::Calendar => vec![("Event", DataType::Object)],
            Self::Tag => vec![
                ("Tag id", DataType::String),
                ("Device id", DataType::String),
            ],
            Self::Conversation => vec![("Sentence", DataType::String), ("Slots", DataType::Object)],
            _ => vec![("Description", DataType::String)],
        };
        pins.extend(
            extra
                .into_iter()
                .enumerate()
                .map(|(i, (name, ty))| data_out(i as u32 + 2, name, ty)),
        );
        NodeDefinition {
            adaptive: false,
            cost: 2,
            type_name: self.type_name().into(),
            title: title.into(),
            category: CATEGORY.into(),
            description: description.into(),
            help: format!(
                "{help}\n\nTriggered sends one pulse per fire, in order, at most one per tick. Info contains all trigger variables. Configure the Home Assistant connection in the plugin settings. Incomplete settings do not subscribe. Invalid stored settings show a node error and log once until fixed. Changes replace the subscription and discard queued fires. Rejected subscriptions retry after 30 seconds. Events missed while disconnected are not replayed; For timers restart when the subscription is recreated."
            ),
            pins,
            properties: self.properties(),
        }
    }
}

#[cfg(test)]
mod tests;
