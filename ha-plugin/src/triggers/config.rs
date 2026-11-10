//! Typed Home Assistant trigger configuration and duration parsing.

use super::Platform;
use chrono::TimeDelta;
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// Parses TickNode duration text plus Home Assistant's HH:MM:SS notation.
pub fn duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    if !text.contains(':') {
        return ticknode_shared::text::parse_duration(text);
    }
    clock_duration(text)
}

fn clock_duration(text: &str) -> Option<Duration> {
    let parts: Vec<_> = text.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours = clock_hours(parts[0])?;
    let minutes = clock_minutes(parts[1])?;
    let seconds = parts[2].parse::<f64>().ok()?;
    if !(0.0..60.0).contains(&seconds) {
        return None;
    }
    Duration::from_secs(
        hours
            .checked_mul(3600)?
            .checked_add(u64::from(minutes) * 60)?,
    )
    .checked_add(Duration::try_from_secs_f64(seconds).ok()?)
}

pub(super) fn signed_duration(text: &str) -> Option<TimeDelta> {
    let text = text.trim();
    let (sign, text) = text
        .strip_prefix('-')
        .map_or((1, text.strip_prefix('+').unwrap_or(text)), |s| (-1, s));
    Some(TimeDelta::from_std(duration(text)?).ok()? * sign)
}

#[derive(Debug, Clone)]
pub struct TriggerConfig {
    pub platform: Platform,
    pub trigger: ha_core_client::triggers::Trigger,
}

impl PartialEq for TriggerConfig {
    fn eq(&self, other: &Self) -> bool {
        self.platform == other.platform
            && serde_json::to_value(&self.trigger).ok() == serde_json::to_value(&other.trigger).ok()
    }
}

mod platforms;

pub(super) struct Properties<'a>(&'a BTreeMap<String, String>);
impl Properties<'_> {
    fn get(&self, key: &str) -> &str {
        self.0.get(key).map(String::as_str).unwrap_or("")
    }
    fn required(&self, key: &str) -> Option<&str> {
        let value = self.get(key);
        (!value.is_empty()).then_some(value)
    }
    fn hold(&self) -> Option<Option<Duration>> {
        if self.get("for").is_empty() {
            Some(None)
        } else {
            duration(self.get("for")).map(Some)
        }
    }
    fn offset(&self) -> Option<Option<TimeDelta>> {
        if self.get("offset").is_empty() {
            Some(None)
        } else {
            signed_duration(self.get("offset")).map(Some)
        }
    }
}

fn event_data(platform: Platform, p: &Properties<'_>) -> Result<Option<Value>, String> {
    if platform != Platform::Event || p.get("event_data").is_empty() {
        return Ok(None);
    }
    let data: Value = serde_json::from_str(p.get("event_data"))
        .map_err(|e| format!("Invalid event data JSON: {e}"))?;
    if !data.is_object() {
        return Err("Event data must be a JSON object".into());
    }
    Ok(Some(data))
}

/// Incomplete or locally invalid settings are quiet. Malformed event JSON gets one actionable log.
pub fn build(
    platform: Platform,
    properties: &BTreeMap<String, String>,
) -> Result<Option<TriggerConfig>, String> {
    let p = Properties(properties);
    let event_data = event_data(platform, &p)?;
    let trigger = match platform {
        Platform::Time => platforms::time(&p),
        Platform::TimePattern => platforms::time_pattern(&p),
        Platform::Sun => platforms::sun(&p),
        Platform::Zone => platforms::zone(&p),
        Platform::Event => platforms::event(&p, &event_data),
        Platform::HomeAssistant => platforms::home_assistant(&p),
        Platform::Mqtt => platforms::mqtt(&p),
        Platform::Webhook => platforms::webhook(&p),
        Platform::Calendar => platforms::calendar(&p),
        Platform::Tag => platforms::tag(&p),
        Platform::Conversation => platforms::conversation(&p),
        Platform::Template => platforms::template(&p),
    };
    Ok(trigger.map(|trigger| TriggerConfig { platform, trigger }))
}

fn clock_hours(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn clock_minutes(text: &str) -> Option<u32> {
    let minutes = text.parse::<u32>().ok()?;
    (minutes < 60).then_some(minutes)
}
