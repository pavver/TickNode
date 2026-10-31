//! Automation trigger definitions, typed configuration and trigger-variable outputs.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use chrono::TimeDelta;
use ha_core_client::{TriggerFired, triggers::*};
use serde_json::Value;
use ticknode_plugin_sdk::{Trigger as NodeTrigger, *};

use crate::convert::{STATE_TYPE, json_to_value, state_value};
use crate::{CATEGORY, text_property, try_with_hub, with_hub};

pub const TRIGGERED: PinId = PinId(0);
pub const INFO: PinId = PinId(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    State,
    NumericState,
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
    Platform::State,
    Platform::NumericState,
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
            Self::State => "ha/trigger_state",
            Self::NumericState => "ha/trigger_numeric_state",
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
            Self::State => (
                "State",
                "Fires when an entity matches a state transition",
                "Choose an entity and optionally an attribute, From and To values. Empty From/To fields impose no filter (attribute-only changes can fire too). For holds the matching state for a duration such as 90, 1h 30m or 00:05:00. From and To outputs contain ha.state values; an absent state clears the corresponding output.",
            ),
            Self::NumericState => (
                "Numeric state",
                "Fires when an entity crosses a numeric threshold",
                "Choose an entity and Above and/or Below thresholds (exclusive). Each threshold is a number or a numeric entity id. Home Assistant fires when the value enters the range, rather than on every update inside it. Attribute selects a numeric attribute; Value template can transform the value in Home Assistant. For requires the range to hold for a duration. Value outputs the numeric To state or selected attribute; it does not evaluate Value template locally and is cleared when nonnumeric. Missing From/To states clear their corresponding outputs.",
            ),
            Self::Time => (
                "Time",
                "Fires at a time or an entity-provided timestamp",
                "At accepts HH:MM[:SS], an input_datetime entity or a timestamp sensor entity. Home Assistant schedules the trigger in its own time zone. A fixed time fires daily.",
            ),
            Self::TimePattern => (
                "Time pattern",
                "Fires when the clock matches a repeating pattern",
                "Set Hours (0–23), Minutes and Seconds (0–59). Use /5 for multiples of five or * for any value. Leave unused fields empty; at least one field must be set. Scheduling and defaults of omitted fields are handled by Home Assistant.",
            ),
            Self::Sun => (
                "Sun",
                "Fires at sunrise or sunset with an optional offset",
                "Choose Sunrise or Sunset. Offset accepts durations such as 30m or 00:30:00; a negative offset such as -00:30:00 fires before the event. Home Assistant uses its configured location.",
            ),
            Self::Zone => (
                "Zone",
                "Fires when a person or device enters or leaves a zone",
                "Choose a person or device tracker, a zone, and Enter or Leave. Home Assistant detects the zone crossing. Entity, From and To describe the tracked entity; an absent state clears the corresponding output.",
            ),
            Self::Event => (
                "Event",
                "Fires when a Home Assistant bus event matches",
                "Event type is the bus event name. Optional Event data is a JSON object whose fields filter the event. Invalid JSON is logged once when edited and prevents subscribing. Event data and Event type outputs describe the received event.",
            ),
            Self::HomeAssistant => (
                "Home Assistant start/stop",
                "Fires when Home Assistant starts or shuts down",
                "Choose Start or Shutdown. The WebSocket must be connected when Home Assistant fires the event; startup events before the connection exists are not replayed. A shutdown pulse may arrive shortly before the connection closes.",
            ),
            Self::Mqtt => (
                "MQTT",
                "Fires when an MQTT message matches a topic",
                "Topic accepts MQTT wildcards. Optional Payload filters by exact payload text. Choose QoS 0, 1 or 2. Home Assistant needs the MQTT integration. Topic and Payload outputs contain the received message.",
            ),
            Self::Webhook => (
                "Webhook",
                "Fires when a Home Assistant webhook receives a request",
                "Set a unique Webhook ID. Send POST or PUT requests to Home Assistant's /api/webhook/<id> endpoint. Local only defaults to Yes. Data contains the JSON body or form data; Query contains URL query parameters. A non-object JSON body is wrapped in a value field. Treat the webhook id as a credential.",
            ),
            Self::Calendar => (
                "Calendar",
                "Fires when a calendar event starts or ends",
                "Choose a calendar entity and Start or End. Optional Offset accepts signed durations; -15m fires fifteen minutes before the event. Event contains the calendar_event trigger variable, including the event summary and times.",
            ),
            Self::Tag => (
                "Tag",
                "Fires when a Home Assistant tag is scanned",
                "Set the Tag ID from Home Assistant. Tag id and Device id outputs identify the scanned tag and scanning device. No device filter is applied.",
            ),
            Self::Conversation => (
                "Sentence",
                "Fires when Assist matches a sentence",
                "Enter one or more sentence patterns separated by semicolons or newlines. Home Assistant's sentence syntax supports optional words and slots. Sentence contains the spoken text and Slots the matched values. Requires a conversation integration.",
            ),
            Self::Template => (
                "Template",
                "Fires when a Home Assistant template becomes true",
                "Value template is a Home Assistant Jinja template. It fires when the result changes from false to true. Optional For requires it to remain true for a duration. Template evaluation and entity tracking run in Home Assistant.",
            ),
        }
    }

    pub fn properties(self) -> Vec<PropertyDefinition> {
        let text = |name, title, description| PropertyDefinition::text(name, title).described(description);
        let entity =
            |source| PropertyDefinition::dynamic("entity_id", "Entity", source).described("Choose an entity or type its entity id");
        let attribute = || text("attribute", "Attribute", "Optional attribute to watch instead of the state");
        let duration = || text("for", "For", "Optional duration: 90, 1h 30m or 00:05:00");
        let offset = || text("offset", "Offset", "Optional signed duration: 30m, -00:30:00 or 0");
        let select =
            |name, title, options: &[(&str, &str)], description| PropertyDefinition::select(name, title, options).described(description);
        match self {
            Self::State => vec![
                entity("entities"),
                attribute(),
                text("from", "From", "Optional previous state or attribute value"),
                text("to", "To", "Optional new state or attribute value"),
                duration(),
            ],
            Self::NumericState => vec![
                entity("entities"),
                attribute(),
                text("above", "Above", "Exclusive lower bound: a number or numeric entity id"),
                text("below", "Below", "Exclusive upper bound: a number or numeric entity id"),
                text("value_template", "Value template", "Optional Home Assistant template to transform the value"),
                duration(),
            ],
            Self::Time => vec![text("at", "At", "HH:MM[:SS], an input_datetime entity or a timestamp sensor entity")],
            Self::TimePattern => vec![
                text("hours", "Hours", "0–23, * or a step such as /2"),
                text("minutes", "Minutes", "0–59, * or a step such as /5"),
                text("seconds", "Seconds", "0–59, * or a step such as /10"),
            ],
            Self::Sun => {
                vec![select("event", "Event", &[("sunrise", "Sunrise"), ("sunset", "Sunset")], "The sun event to wait for"), offset()]
            }
            Self::Zone => vec![
                entity("persons"),
                PropertyDefinition::dynamic("zone", "Zone", "zones").described("The zone to enter or leave"),
                select("event", "Event", &[("enter", "Enter"), ("leave", "Leave")], "The zone crossing to wait for"),
            ],
            Self::Event => vec![
                text("event_type", "Event type", "Home Assistant bus event name"),
                text("event_data", "Event data", "Optional JSON object used to filter event fields"),
            ],
            Self::HomeAssistant => {
                vec![select("event", "Event", &[("start", "Start"), ("shutdown", "Shutdown")], "The Home Assistant lifecycle event")]
            }
            Self::Mqtt => vec![
                text("topic", "Topic", "MQTT topic, including optional wildcards"),
                text("payload", "Payload", "Optional exact payload filter"),
                select("qos", "QoS", &[("0", "0"), ("1", "1"), ("2", "2")], "MQTT quality of service"),
            ],
            Self::Webhook => vec![
                text("webhook_id", "Webhook ID", "Unique identifier for /api/webhook/<id>"),
                select("local_only", "Local only", &[("yes", "Yes"), ("no", "No")], "Only accept requests from the local network"),
            ],
            Self::Calendar => vec![
                entity("calendars"),
                select("event", "Event", &[("start", "Start"), ("end", "End")], "The calendar event boundary"),
                offset(),
            ],
            Self::Tag => vec![text("tag_id", "Tag ID", "The Home Assistant tag identifier")],
            Self::Conversation => vec![text("command", "Sentence", "Sentence patterns separated by semicolons or newlines")],
            Self::Template => vec![text("value_template", "Value template", "Home Assistant Jinja template that becomes true"), duration()],
        }
    }

    pub fn defaults(self) -> BTreeMap<String, String> {
        self.properties()
            .into_iter()
            .map(|p| {
                let value = match &p.kind {
                    PropertyKind::Select(options) => options.first().map(|(v, _)| v.clone()).unwrap_or_default(),
                    _ => String::new(),
                };
                (p.name, value)
            })
            .collect()
    }

    pub fn definition(self) -> NodeDefinition {
        let (title, description, help) = self.texts();
        let mut pins = vec![exec_out(0, "Triggered"), data_out(1, "Info", DataType::Object)];
        let extra: Vec<(&str, DataType)> = match self {
            Self::State | Self::NumericState | Self::Zone => {
                let mut pins = vec![
                    ("Entity", DataType::String),
                    ("From", DataType::Named(STATE_TYPE.into())),
                    ("To", DataType::Named(STATE_TYPE.into())),
                ];
                if self == Self::NumericState {
                    pins.push(("Value", DataType::Number));
                }
                pins
            }
            Self::Event => vec![("Event data", DataType::Object), ("Event type", DataType::String)],
            Self::Mqtt => vec![("Topic", DataType::String), ("Payload", DataType::String)],
            Self::Webhook => vec![("Data", DataType::Object), ("Query", DataType::Object)],
            Self::Calendar => vec![("Event", DataType::Object)],
            Self::Tag => vec![("Tag id", DataType::String), ("Device id", DataType::String)],
            Self::Conversation => vec![("Sentence", DataType::String), ("Slots", DataType::Object)],
            _ => vec![("Description", DataType::String)],
        };
        pins.extend(extra.into_iter().enumerate().map(|(i, (name, ty))| data_out(i as u32 + 2, name, ty)));
        NodeDefinition {
            type_name: self.type_name().into(),
            title: title.into(),
            category: CATEGORY.into(),
            description: description.into(),
            help: format!(
                "{help}\n\nTriggered sends one pulse per fire, in order, at most one per tick. Info contains all trigger variables. Configure the Home Assistant connection in the plugin settings. Incomplete settings do not subscribe. Changes replace the subscription and discard queued fires. Rejected subscriptions retry after 30 seconds. Events missed while disconnected are not replayed; For timers restart when the subscription is recreated."
            ),
            pins,
            properties: self.properties(),
        }
    }
}

/// Parses TickNode duration text plus Home Assistant's HH:MM:SS notation.
pub fn duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    if !text.contains(':') {
        return ticknode_shared::text::parse_duration(text);
    }
    let parts: Vec<_> = text.split(':').collect();
    if parts.len() != 3 || !parts[0].bytes().all(|b| b.is_ascii_digit()) || parts[0].is_empty() {
        return None;
    }
    let hours = parts[0].parse::<u64>().ok()?;
    let minutes = parts[1].parse::<u32>().ok()?;
    let seconds = parts[2].parse::<f64>().ok()?;
    if minutes >= 60 || !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Duration::from_secs(hours.checked_mul(3600)?.checked_add(u64::from(minutes) * 60)?)
        .checked_add(Duration::try_from_secs_f64(seconds).ok()?)
}

fn signed_duration(text: &str) -> Option<TimeDelta> {
    let text = text.trim();
    let (sign, text) = text.strip_prefix('-').map_or((1, text.strip_prefix('+').unwrap_or(text)), |s| (-1, s));
    Some(TimeDelta::from_std(duration(text)?).ok()? * sign)
}

#[derive(Debug, Clone)]
pub struct TriggerConfig {
    pub platform: Platform,
    pub trigger: ha_core_client::triggers::Trigger,
    pub attribute: String,
}

impl PartialEq for TriggerConfig {
    fn eq(&self, other: &Self) -> bool {
        self.platform == other.platform
            && self.attribute == other.attribute
            && serde_json::to_value(&self.trigger).ok() == serde_json::to_value(&other.trigger).ok()
    }
}

/// Incomplete or locally invalid settings are quiet. Malformed event JSON gets one actionable log.
pub fn build(platform: Platform, properties: &BTreeMap<String, String>) -> Result<Option<TriggerConfig>, String> {
    let p = |name: &str| properties.get(name).map(String::as_str).unwrap_or("");
    let event_data = if platform == Platform::Event && !p("event_data").is_empty() {
        let data: Value = serde_json::from_str(p("event_data")).map_err(|e| format!("Invalid event data JSON: {e}"))?;
        if !data.is_object() {
            return Err("Event data must be a JSON object".into());
        }
        Some(data)
    } else {
        None
    };
    let valid = || -> Option<ha_core_client::triggers::Trigger> {
        let required = |key| (!p(key).is_empty()).then(|| p(key));
        let hold = || if p("for").is_empty() { Some(None) } else { duration(p("for")).map(Some) };
        let offset = || if p("offset").is_empty() { Some(None) } else { signed_duration(p("offset")).map(Some) };
        Some(match platform {
            Platform::State => {
                let mut t = StateTrigger::new(required("entity_id")?);
                if !p("attribute").is_empty() {
                    t = t.attribute(p("attribute"));
                }
                if !p("from").is_empty() {
                    t = t.from(p("from"));
                }
                if !p("to").is_empty() {
                    t = t.to(p("to"));
                }
                if let Some(d) = hold()? {
                    t = t.for_duration(d);
                }
                t.into()
            }
            Platform::NumericState => {
                let mut t = NumericStateTrigger::new(required("entity_id")?);
                if p("above").is_empty() && p("below").is_empty() {
                    return None;
                }
                if !p("above").is_empty() {
                    t = match p("above").parse::<f64>() {
                        Ok(n) if n.is_finite() => t.above(n),
                        Ok(_) => return None,
                        Err(_) => t.above_entity(p("above")),
                    };
                }
                if !p("below").is_empty() {
                    t = match p("below").parse::<f64>() {
                        Ok(n) if n.is_finite() => t.below(n),
                        Ok(_) => return None,
                        Err(_) => t.below_entity(p("below")),
                    };
                }
                if !p("attribute").is_empty() {
                    t = t.attribute(p("attribute"));
                }
                if !p("value_template").is_empty() {
                    t = t.value_template(p("value_template"));
                }
                if let Some(d) = hold()? {
                    t = t.for_duration(d);
                }
                t.into()
            }
            Platform::Time => {
                let at = required("at")?;
                if ticknode_shared::text::parse_time(at).is_none()
                    && !((at.starts_with("input_datetime.") || at.starts_with("sensor.")) && !at.ends_with('.'))
                {
                    return None;
                }
                TimeTrigger::new(at).into()
            }
            Platform::TimePattern => {
                if ["hours", "minutes", "seconds"].iter().all(|k| p(k).is_empty()) {
                    return None;
                }
                let mut t = TimePatternTrigger::new();
                for (key, max) in [("hours", 23), ("minutes", 59), ("seconds", 59)] {
                    let v = p(key);
                    if v.is_empty() {
                        continue;
                    }
                    let pattern_ok = v == "*"
                        || v.strip_prefix('/').map_or_else(
                            || v.parse::<u32>().is_ok_and(|n| n <= max),
                            |s| s.parse::<u32>().is_ok_and(|n| n > 0 && n <= max + 1),
                        );
                    if !pattern_ok {
                        return None;
                    }
                    t = match key {
                        "hours" => t.hours(v),
                        "minutes" => t.minutes(v),
                        _ => t.seconds(v),
                    };
                }
                t.into()
            }
            Platform::Sun => {
                let mut t = SunTrigger::new(match p("event") {
                    "sunrise" => SunEvent::Sunrise,
                    "sunset" => SunEvent::Sunset,
                    _ => return None,
                });
                if let Some(d) = offset()? {
                    t = t.offset(d);
                }
                t.into()
            }
            Platform::Zone => ZoneTrigger::new(
                required("entity_id")?,
                required("zone")?,
                match p("event") {
                    "enter" => ZoneEvent::Enter,
                    "leave" => ZoneEvent::Leave,
                    _ => return None,
                },
            )
            .into(),
            Platform::Event => {
                let mut t = EventTrigger::new(required("event_type")?);
                if let Some(data) = &event_data {
                    t = t.event_data(data).ok()?;
                }
                t.into()
            }
            Platform::HomeAssistant => HomeAssistantTrigger::new(match p("event") {
                "start" => HomeAssistantEvent::Start,
                "shutdown" => HomeAssistantEvent::Shutdown,
                _ => return None,
            })
            .into(),
            Platform::Mqtt => {
                let mut t = MqttTrigger::new(required("topic")?).qos(match p("qos") {
                    "0" => 0,
                    "1" => 1,
                    "2" => 2,
                    _ => return None,
                });
                if !p("payload").is_empty() {
                    t = t.payload(p("payload"));
                }
                t.into()
            }
            Platform::Webhook => WebhookTrigger::new(required("webhook_id")?)
                .local_only(match p("local_only") {
                    "yes" => true,
                    "no" => false,
                    _ => return None,
                })
                .into(),
            Platform::Calendar => {
                let mut t = CalendarTrigger::new(
                    required("entity_id")?,
                    match p("event") {
                        "start" => CalendarEvent::Start,
                        "end" => CalendarEvent::End,
                        _ => return None,
                    },
                );
                if let Some(d) = offset()? {
                    t = t.offset(d);
                }
                t.into()
            }
            Platform::Tag => TagTrigger::new(required("tag_id")?).into(),
            Platform::Conversation => {
                let mut commands = p("command").split([';', '\n']).map(str::trim).filter(|s| !s.is_empty());
                let mut t = ConversationTrigger::new(commands.next()?);
                for command in commands {
                    t = t.and_command(command);
                }
                t.into()
            }
            Platform::Template => {
                let mut t = TemplateTrigger::new(required("value_template")?);
                if let Some(d) = hold()? {
                    t = t.for_duration(d);
                }
                t.into()
            }
        })
    };
    Ok(valid().map(|trigger| TriggerConfig { platform, trigger, attribute: p("attribute").into() }))
}

fn object(value: &Value) -> DataValue {
    let json = match value {
        Value::Object(_) => value.clone(),
        Value::Null => serde_json::json!({}),
        _ => serde_json::json!({"value": value}),
    };
    json_to_value(&json).expect("an object always converts")
}

pub fn outputs(config: &TriggerConfig, fired: &TriggerFired) -> Vec<Output> {
    let raw = &fired.trigger;
    let mut outputs = vec![Output::Data(INFO, object(raw))];
    let string = |value: &Value| DataValue::from(value.as_str().unwrap_or_default());
    let values = match config.platform {
        Platform::State | Platform::NumericState | Platform::Zone => {
            let entity = fired.entity_id().unwrap_or_default();
            let to = fired.to_state();
            let mut values = vec![
                Some(DataValue::from(entity)),
                fired.from_state().as_ref().map(state_value),
                to.as_ref().map(state_value),
            ];
            if config.platform == Platform::NumericState {
                let number = to
                    .as_ref()
                    .and_then(|state| {
                        if config.attribute.is_empty() {
                            state.state.parse::<f64>().ok()
                        } else {
                            state.attributes.get(&config.attribute).and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok()))
                        }
                    })
                    .filter(|n| n.is_finite());
                values.push(number.map(DataValue::from));
            }
            outputs.extend(values.into_iter().enumerate().map(|(i, v)| {
                let pin = PinId(i as u32 + 2);
                v.map_or(Output::Clear(pin), |value| Output::Data(pin, value))
            }));
            outputs.push(Output::Exec(TRIGGERED));
            return outputs;
        }
        Platform::Event => vec![object(&raw["event"]["data"]), string(&raw["event"]["event_type"])],
        Platform::Mqtt => vec![string(&raw["topic"]), string(&raw["payload"])],
        Platform::Webhook => vec![object(raw.get("json").filter(|v| !v.is_null()).unwrap_or(&raw["data"])), object(&raw["query"])],
        Platform::Calendar => vec![object(&raw["calendar_event"])],
        Platform::Tag => vec![string(&raw["event"]["data"]["tag_id"]), string(&raw["event"]["data"]["device_id"])],
        Platform::Conversation => vec![string(&raw["sentence"]), object(&raw["slots"])],
        _ => vec![DataValue::from(fired.description().unwrap_or_default())],
    };
    outputs.extend(values.into_iter().enumerate().map(|(i, v)| Output::Data(PinId(i as u32 + 2), v)));
    outputs.push(Output::Exec(TRIGGERED));
    outputs
}

pub struct TriggerNode {
    node: NodeId,
    serial: Option<u64>,
    platform: Platform,
    properties: BTreeMap<String, String>,
    local_error: Option<String>,
}

impl TriggerNode {
    pub fn new(platform: Platform, node: NodeId) -> Self {
        let properties = platform.defaults();
        let serial = (!node.0.is_nil()).then(|| {
            with_hub(|hub, env| {
                let serial = hub.add_trigger(node, Instant::now());
                hub.set_trigger(node, serial, build(platform, &properties).ok().flatten(), Instant::now(), env);
                serial
            })
        });
        Self { node, serial, platform, properties, local_error: None }
    }
}

impl Drop for TriggerNode {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_trigger(self.node, serial, Instant::now(), env));
        }
    }
}

impl PluginNode for TriggerNode {
    fn definition(&self) -> NodeDefinition {
        self.platform.definition()
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.properties.iter().map(|(k, v)| (k.clone(), DataValue::from(v.as_str()))).collect()
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else { return };
        let Some(current) = self.properties.get(name) else { return };
        if current == &text {
            return;
        }
        if let Some(def) = self.platform.properties().iter().find(|p| p.name == name)
            && let PropertyKind::Select(options) = &def.kind
            && !options.iter().any(|(v, _)| v == &text)
        {
            return;
        }
        self.properties.insert(name.into(), text);
        self.local_error = None;
        let config = match build(self.platform, &self.properties) {
            Ok(config) => config,
            Err(error) => {
                self.local_error = Some(error);
                None
            }
        };
        if let Some(serial) = self.serial {
            with_hub(|hub, env| hub.set_trigger(self.node, serial, config, Instant::now(), env));
        }
    }
    fn execute(&mut self, _: NodeTrigger, _: &Inputs) -> Vec<Output> {
        Vec::new()
    }
    fn polls(&self) -> bool {
        true
    }
    fn poll(&mut self, _: u64) -> Option<PinId> {
        if let Some(error) = self.local_error.take() {
            host::error(&error);
        }
        if let Some(serial) = self.serial {
            let errors = with_hub(|hub, _| hub.take_trigger_errors(self.node, serial));
            for error in errors {
                host::error(&error);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
