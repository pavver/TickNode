//! HA-style gates. All evaluations except Jinja templates use the shared live cache.
use std::collections::BTreeMap;
use std::time::Instant;

use chrono::{DateTime, Datelike, NaiveTime, TimeDelta, Utc};
use serde_json::Value;
use ticknode_plugin_sdk::*;

use crate::{CATEGORY, live::Live, text_property, try_with_hub, with_hub};

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
    State,
    NumericState,
    Time,
    Sun,
    Zone,
    Template,
}
pub const KINDS: &[Kind] = &[
    Kind::State,
    Kind::NumericState,
    Kind::Time,
    Kind::Sun,
    Kind::Zone,
    Kind::Template,
];
impl Kind {
    pub fn type_name(self) -> &'static str {
        match self {
            Self::State => "ha/condition_state",
            Self::NumericState => "ha/condition_numeric_state",
            Self::Time => "ha/condition_time",
            Self::Sun => "ha/condition_sun",
            Self::Zone => "ha/condition_zone",
            Self::Template => "ha/condition_template",
        }
    }
    pub fn definition(self) -> NodeDefinition {
        let text =
            |name, title, description| PropertyDefinition::text(name, title).described(description);
        let entity = || {
            PropertyDefinition::dynamic("entity_id", "Entity", "entities")
                .described("Entity to test using the live state cache")
        };
        let attribute = || {
            text(
                "attribute",
                "Attribute",
                "Optional attribute; empty tests the entity state",
            )
        };
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
            Self::State => (
                "State condition",
                "Checks an entity state or attribute",
                "State accepts comma-separated alternatives (any of). Attribute optionally selects an attribute. For accepts 90, 1h 30m or 00:05:00 and checks the entity's last_changed, also when testing an attribute; it does not track attribute-only duration. An empty State matches an empty string.",
                vec![
                    entity(),
                    attribute(),
                    text(
                        "state",
                        "State",
                        "Expected state or comma-separated alternatives",
                    ),
                    text(
                        "for",
                        "For",
                        "Optional minimum duration based on last_changed",
                    ),
                ],
            ),
            Self::NumericState => (
                "Numeric state condition",
                "Checks exclusive numeric thresholds",
                "Tests the entity state or selected numeric attribute. Above and Below are exclusive bounds, each a finite number or an entity id whose current state is numeric. At least one bound is required. Value templates are not evaluated locally.",
                vec![
                    entity(),
                    attribute(),
                    text(
                        "above",
                        "Above",
                        "Optional exclusive lower bound: number or numeric entity id",
                    ),
                    text(
                        "below",
                        "Below",
                        "Optional exclusive upper bound: number or numeric entity id",
                    ),
                ],
            ),
            Self::Time => (
                "Time condition",
                "Checks local time and weekdays in Home Assistant",
                "After is inclusive; Before is exclusive. Equal After/Before times cover the full day. A window crossing midnight uses either side of midnight; weekdays always refer to the current local day. Weekday accepts English mon,tue,wed,thu,fri,sat,sun. Empty fields impose no restriction. Uses get_config time_zone and a live HA-rendered UTC offset, never the WASI host timezone. HA refreshes the offset every minute: a timezone/DST change can lag by up to one minute plus event delivery. Until timezone data arrives the result is No.",
                vec![
                    text("after", "After", "Optional local HH:MM[:SS], inclusive"),
                    text("before", "Before", "Optional local HH:MM[:SS], exclusive"),
                    text(
                        "weekday",
                        "Weekday",
                        "Optional comma-separated mon,tue,wed,thu,fri,sat,sun",
                    ),
                ],
            ),
            Self::Sun => (
                "Sun condition",
                "Checks sunrise and sunset boundaries with offsets",
                "Uses cached sun.sun next_rising/next_setting. Each next event's HA-local wall time is projected onto today's local date, then the signed offset is added. Before and After are inclusive, independent tests combined with AND, as in HA (After sunset plus Before sunrise does not form an overnight window). Projection is an approximation after an event has passed: tomorrow's solar time may differ by a few minutes. Polar events more than two days away fail to No. Offsets accept -30m or 00:30:00. Uses the same HA UTC offset and minute refresh limitation as Time.",
                vec![
                    sun("after", "After"),
                    text(
                        "after_offset",
                        "After offset",
                        "Optional signed duration added to the After event",
                    ),
                    sun("before", "Before"),
                    text(
                        "before_offset",
                        "Before offset",
                        "Optional signed duration added to the Before event",
                    ),
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
                vec![text(
                    "value_template",
                    "Value template",
                    "Home Assistant Jinja template whose result is boolean",
                )],
            ),
        };
        NodeDefinition {
            type_name: self.type_name().into(),
            title: title.into(),
            description: description.into(),
            help: format!(
                "{help}\n\nIn evaluates the condition. Result is boolean; exactly one of Yes or No fires. Local conditions make no request on execution. Unknown, unavailable, not yet loaded or disconnected states yield No and a warning once until that situation changes."
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

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|v| v.is_finite())
}
fn scalar(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}
fn offset(text: &str) -> Result<TimeDelta, String> {
    if text.is_empty() {
        return Ok(TimeDelta::zero());
    }
    let (sign, text) = text
        .strip_prefix('-')
        .map_or((1, text.strip_prefix('+').unwrap_or(text)), |s| (-1, s));
    crate::triggers::duration(text)
        .and_then(|d| TimeDelta::from_std(d).ok())
        .map(|d| d * sign)
        .ok_or_else(|| "Invalid duration".into())
}
fn clock(text: &str) -> Result<Option<NaiveTime>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    NaiveTime::parse_from_str(text, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M"))
        .map(Some)
        .map_err(|_| format!("Invalid time: {text}"))
}

pub fn evaluate(
    kind: Kind,
    p: &BTreeMap<String, String>,
    live: &Live,
    now: DateTime<Utc>,
) -> Result<bool, String> {
    let p = |key: &str| p.get(key).map(String::as_str).unwrap_or_default();
    let value = |entity: &str, attr: &str| -> Result<Value, String> {
        let state = live.state(entity)?;
        if attr.is_empty() {
            Ok(Value::String(state.state.clone()))
        } else {
            state
                .attributes
                .get(attr)
                .ok_or_else(|| format!("Entity {entity} has no attribute {attr}"))
        }
    };
    match kind {
        Kind::State => {
            let actual = scalar(&value(p("entity_id"), p("attribute"))?);
            let held = if p("for").is_empty() {
                true
            } else {
                if p("for").starts_with('-') {
                    return Err("For must not be negative".into());
                }
                let hold = offset(p("for"))?;
                now.signed_duration_since(live.state(p("entity_id"))?.last_changed) >= hold
            };
            Ok(held
                && p("state")
                    .split(',')
                    .map(str::trim)
                    .any(|expected| expected == actual))
        }
        Kind::NumericState => {
            let actual = number(&value(p("entity_id"), p("attribute"))?)
                .ok_or("Condition value is not a finite number")?;
            let bound = |text: &str| -> Result<Option<f64>, String> {
                if text.is_empty() {
                    return Ok(None);
                }
                if let Ok(n) = text.parse::<f64>() {
                    return n
                        .is_finite()
                        .then_some(Some(n))
                        .ok_or_else(|| "Numeric bound must be finite".into());
                }
                number(&value(text, "")?)
                    .map(Some)
                    .ok_or_else(|| format!("Bound entity {text} is not numeric"))
            };
            let above = bound(p("above"))?;
            let below = bound(p("below"))?;
            if above.is_none() && below.is_none() {
                return Err("Numeric condition needs Above or Below".into());
            }
            Ok(above.is_none_or(|n| actual > n) && below.is_none_or(|n| actual < n))
        }
        Kind::Time => {
            let local = now.with_timezone(&live.offset()?);
            let after = clock(p("after"))?;
            let before = clock(p("before"))?;
            let mut weekdays = Vec::new();
            for day in p("weekday")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                weekdays.push(
                    ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                        .iter()
                        .position(|s| s.eq_ignore_ascii_case(day))
                        .ok_or_else(|| format!("Invalid weekday: {day}"))?,
                );
            }
            let time = local.time();
            let within = match (after, before) {
                (Some(a), Some(b)) if a >= b => time >= a || time < b,
                (a, b) => a.is_none_or(|a| time >= a) && b.is_none_or(|b| time < b),
            };
            Ok(within
                && (weekdays.is_empty()
                    || weekdays.contains(&(local.weekday().num_days_from_monday() as usize))))
        }
        Kind::Sun => {
            let zone = live.offset()?;
            let local = now.with_timezone(&zone);
            let boundary = |event: &str,
                            delta: &str|
             -> Result<Option<DateTime<chrono::FixedOffset>>, String> {
                if event == "none" || event.is_empty() {
                    return Ok(None);
                }
                let attr = match event {
                    "sunrise" => "next_rising",
                    "sunset" => "next_setting",
                    _ => return Err("Invalid sun event".into()),
                };
                let next = value("sun.sun", attr)?;
                let next =
                    DateTime::parse_from_rfc3339(next.as_str().ok_or("Invalid solar timestamp")?)
                        .map_err(|_| "Invalid solar timestamp")?;
                if (next.with_timezone(&Utc) - now).num_seconds().abs() > 2 * 86400 {
                    return Err("Solar event is too far away (polar day/night)".into());
                }
                let today = local
                    .date_naive()
                    .and_time(next.with_timezone(&zone).time())
                    .and_local_timezone(zone)
                    .single()
                    .ok_or("Invalid solar date")?;
                Ok(Some(
                    today
                        .checked_add_signed(offset(delta)?)
                        .ok_or("Solar offset is out of range")?,
                ))
            };
            let after = boundary(p("after"), p("after_offset"))?;
            let before = boundary(p("before"), p("before_offset"))?;
            Ok(after.is_none_or(|a| local >= a) && before.is_none_or(|b| local <= b))
        }
        Kind::Zone => {
            let person = live.state(p("entity_id"))?;
            let zone = live.state(p("zone"))?;
            let name = if p("zone") == "zone.home" {
                "home".to_owned()
            } else {
                zone.attributes
                    .get("friendly_name")
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .ok_or("Zone has no friendly_name")?
            };
            Ok(person.state == name)
        }
        Kind::Template => Err("Template conditions require asynchronous evaluation".into()),
    }
}

pub fn template_bool(value: &Value) -> bool {
    // HA template conditions accept only the rendered text "true" (case-insensitive).
    value.as_bool().unwrap_or_else(|| {
        value
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("true"))
    })
}

pub struct ConditionNode {
    node: NodeId,
    serial: Option<u64>,
    kind: Kind,
    properties: BTreeMap<String, String>,
    warning: Option<(u64, String)>,
}
impl ConditionNode {
    pub fn new(kind: Kind, node: NodeId) -> Self {
        let properties = kind
            .definition()
            .properties
            .iter()
            .map(|p| {
                (
                    p.name.clone(),
                    if kind == Kind::Sun && matches!(p.name.as_str(), "after" | "before") {
                        "none".into()
                    } else {
                        String::new()
                    },
                )
            })
            .collect();
        let serial = (!node.0.is_nil()).then(|| with_hub(|hub, _| hub.add_condition(node)));
        Self {
            node,
            serial,
            kind,
            properties,
            warning: None,
        }
    }
    fn warn(&mut self, generation: u64, error: String) {
        let warning = (generation, error);
        if self.warning.as_ref() != Some(&warning) {
            host::warn(&format!(
                "{}: {}; returning No",
                self.kind.type_name(),
                warning.1
            ));
            self.warning = Some(warning);
        }
    }
}
impl Drop for ConditionNode {
    fn drop(&mut self) {
        if let Some(serial) = self.serial {
            try_with_hub(|hub, env| hub.remove_condition(self.node, serial, Instant::now(), env));
        }
    }
}
impl PluginNode for ConditionNode {
    fn definition(&self) -> NodeDefinition {
        self.kind.definition()
    }
    fn properties(&self) -> Vec<(String, DataValue)> {
        self.properties
            .iter()
            .map(|(k, v)| (k.clone(), DataValue::from(v.as_str())))
            .collect()
    }
    fn set_property(&mut self, name: &str, value: DataValue) {
        let Some(text) = text_property(value) else {
            return;
        };
        let Some(old) = self.properties.get(name) else {
            return;
        };
        if old == &text {
            return;
        }
        if let Some(def) = self
            .kind
            .definition()
            .properties
            .iter()
            .find(|p| p.name == name)
            && let PropertyKind::Select(options) = &def.kind
            && !options.iter().any(|(v, _)| v == &text)
        {
            return;
        }
        self.properties.insert(name.into(), text);
        self.warning = None;
        if let Some(serial) = self.serial {
            with_hub(|hub, env| hub.cancel_templates(self.node, serial, Instant::now(), env));
        }
    }
    fn execute(&mut self, _: Trigger, _: &Inputs) -> Vec<Output> {
        if self.kind == Kind::Template {
            return with_hub(|hub, env| {
                hub.render_condition(
                    self.node,
                    self.serial.unwrap_or_default(),
                    &self.properties["value_template"],
                    Instant::now(),
                    env,
                )
            });
        }
        let (generation, result) = with_hub(|hub, _| {
            (
                hub.live.generation,
                evaluate(self.kind, &self.properties, &hub.live, Utc::now()),
            )
        });
        match result {
            Ok(value) => {
                self.warning = None;
                outputs(value)
            }
            Err(error) => {
                self.warn(generation, error);
                outputs(false)
            }
        }
    }
}

#[cfg(test)]
mod tests;
