//! Typed Home Assistant trigger configuration and duration parsing.

use std::collections::BTreeMap;
use std::time::Duration;
use chrono::TimeDelta;
use ha_core_client::triggers::*;
use serde_json::Value;
use super::Platform;

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

pub(super) fn signed_duration(text: &str) -> Option<TimeDelta> {
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
