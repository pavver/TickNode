//! Property validation shared by interactive edits and legacy runtime configurations.
use ticknode_plugin_sdk::*;

pub(crate) fn error(key: &str, fallback: &str) -> UiError {
    UiError::new(format!("home-assistant-error-{key}"), fallback)
}

pub(crate) fn schema(def: &NodeDefinition, name: &str, value: &DataValue) -> Result<(), UiError> {
    let Some(property) = def.properties.iter().find(|p| p.name == name) else {
        return Ok(());
    };
    let DataValue::String(text) = value else {
        return Err(error("property-text", "The property must be text"));
    };
    if let PropertyKind::Select(options) = &property.kind {
        if !options.iter().any(|(v, _)| v == text) {
            return Err(error("property-option", "Choose an available option"));
        }
    }
    ticknode_shared::property::normalize_property_value(&property.kind, value).map_err(|_| {
        let (key, fallback) = match property.kind {
            PropertyKind::Duration { signed: true } => (
                "signed-duration",
                "Enter a signed duration, for example -30m or 00:30:00",
            ),
            PropertyKind::Duration { signed: false } => (
                "duration",
                "Enter a non-negative duration, for example 5m or 00:05:00",
            ),
            PropertyKind::Time => ("time", "Enter a time as HH:MM or HH:MM:SS"),
            PropertyKind::MultiSelect(_) => ("weekdays", "Choose weekdays from Monday to Sunday"),
            _ => ("property-text", "The property must be text"),
        };
        error(key, fallback)
    })?;
    if matches!(property.kind, PropertyKind::Duration { signed: true }) && !text.trim().is_empty() {
        let magnitude = text.trim().trim_start_matches(['-', '+']);
        let fits = ticknode_shared::property::parse_property_duration(magnitude)
            .is_some_and(|d| chrono::TimeDelta::from_std(d).is_ok());
        if !fits {
            return Err(error(
                "signed-duration",
                "Enter a signed duration, for example -30m or 00:30:00",
            ));
        }
    }
    Ok(())
}

pub(crate) fn entity_id(value: &str) -> bool {
    value.split_once('.').is_some_and(|(domain, object)| {
        !domain.is_empty()
            && !object.is_empty()
            && domain
                .bytes()
                .chain(object.bytes())
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
    })
}

/// NUL and overlong filters are forbidden; empty topic levels remain valid MQTT levels.
pub(crate) fn mqtt_topic(value: &str) -> bool {
    if value.is_empty() || value.len() > 65_535 || value.contains('\0') {
        return false;
    }
    let levels: Vec<_> = value.split('/').collect();
    levels.iter().enumerate().all(|(i, level)| {
        (!level.contains('#') || (*level == "#" && i + 1 == levels.len()))
            && (!level.contains('+') || *level == "+")
    })
}

pub(crate) fn pattern(value: &str, max: u32) -> bool {
    if value == "*" {
        return true;
    }
    let (step, text) = value
        .strip_prefix('/')
        .map_or((false, value), |v| (true, v));
    !text.is_empty()
        && text.bytes().all(|c| c.is_ascii_digit())
        && text
            .parse::<u32>()
            .is_ok_and(|n| n <= max && (!step || n > 0))
}

pub(crate) fn trigger(
    platform: crate::triggers::Platform,
    name: &str,
    value: &DataValue,
) -> Result<(), UiError> {
    use crate::triggers::Platform;
    schema(&platform.definition(), name, value)?;
    let DataValue::String(text) = value else {
        return Ok(());
    };
    // Empty fields are unset; a new or partially configured node remains quiet.
    if text.is_empty() {
        return Ok(());
    }
    let (valid, key, fallback) = match (platform, name) {
        (Platform::Event, "event_data") => (
            serde_json::from_str::<serde_json::Value>(text).is_ok_and(|v| v.is_object()),
            "event-data",
            "Event data must be a JSON object",
        ),
        (Platform::Event, "event_type") => (
            !text.chars().any(char::is_whitespace),
            "event-type",
            "Event type must not contain whitespace",
        ),
        (Platform::Mqtt, "topic") => (
            mqtt_topic(text),
            "mqtt-topic",
            "Invalid MQTT topic filter: + must occupy a whole level and # must be the last whole level",
        ),
        (Platform::Webhook, "webhook_id") => (
            text.bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-')),
            "webhook-id",
            "Webhook ID may contain only letters, digits, underscores and hyphens",
        ),
        (Platform::TimePattern, "hours" | "minutes" | "seconds") => (
            pattern(text, if name == "hours" { 23 } else { 59 }),
            "time-pattern",
            "Use a number, * or /n; hours range from 0 to 23, minutes and seconds from 0 to 59; n must be positive",
        ),
        (Platform::Time, "at") => (
            ticknode_shared::text::parse_time(text).is_some() || entity_id(text),
            "time-at",
            "Enter HH:MM[:SS] or an entity ID such as input_datetime.alarm or sensor.next_alarm",
        ),
        (Platform::Zone | Platform::Calendar, "entity_id" | "zone") => (
            entity_id(text),
            "entity-id",
            "Enter an entity ID as domain.object_id",
        ),
        _ => return Ok(()),
    };
    if valid {
        Ok(())
    } else {
        Err(error(key, fallback))
    }
}

pub(crate) fn condition(
    kind: crate::conditions::Kind,
    name: &str,
    value: &DataValue,
) -> Result<(), UiError> {
    if kind == crate::conditions::Kind::Sun
        && matches!(name, "after" | "before")
        && *value == DataValue::from("")
    {
        return Ok(());
    }
    schema(&kind.definition(), name, value)?;
    if matches!(name, "entity_id" | "zone")
        && let DataValue::String(text) = value
        && !text.is_empty()
        && !entity_id(text)
    {
        return Err(error("entity-id", "Enter an entity ID as domain.object_id"));
    }
    Ok(())
}

pub(crate) fn stored_trigger(
    platform: crate::triggers::Platform,
    properties: &std::collections::BTreeMap<String, String>,
) -> Result<(), UiError> {
    for (name, value) in properties {
        trigger(platform, name, &DataValue::from(value.as_str()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
