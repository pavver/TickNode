//! Platform-specific trigger construction.
use super::Properties;
use ha_core_client::triggers::*;
use serde_json::Value;

pub(super) fn time(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let at = p.required("at")?;
        if ticknode_shared::text::parse_time(at).is_none() && !crate::validation::entity_id(at) {
            return None;
        }
        TimeTrigger::new(at).into()
    })
}

pub(super) fn time_pattern(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        if ["hours", "minutes", "seconds"]
            .iter()
            .all(|k| p.get(k).is_empty())
        {
            return None;
        }
        let mut t = TimePatternTrigger::new();
        for (key, max) in [("hours", 23), ("minutes", 59), ("seconds", 59)] {
            let v = p.get(key);
            if v.is_empty() {
                continue;
            }

            if !crate::validation::pattern(v, max) {
                return None;
            }
            t = match key {
                "hours" => t.hours(v),
                "minutes" => t.minutes(v),
                _ => t.seconds(v),
            };
        }
        t.into()
    })
}

pub(super) fn sun(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut t = SunTrigger::new(match p.get("event") {
            "sunrise" => SunEvent::Sunrise,
            "sunset" => SunEvent::Sunset,
            _ => return None,
        });
        if let Some(d) = p.offset()? {
            t = t.offset(d);
        }
        t.into()
    })
}

pub(super) fn zone(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some(
        ZoneTrigger::new(
            p.required("entity_id")?,
            p.required("zone")?,
            match p.get("event") {
                "enter" => ZoneEvent::Enter,
                "leave" => ZoneEvent::Leave,
                _ => return None,
            },
        )
        .into(),
    )
}

pub(super) fn event(
    p: &Properties<'_>,
    event_data: &Option<Value>,
) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut t = EventTrigger::new(p.required("event_type")?);
        if let Some(data) = event_data {
            t = t.event_data(data).ok()?;
        }
        t.into()
    })
}

pub(super) fn home_assistant(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some(
        HomeAssistantTrigger::new(match p.get("event") {
            "start" => HomeAssistantEvent::Start,
            "shutdown" => HomeAssistantEvent::Shutdown,
            _ => return None,
        })
        .into(),
    )
}

pub(super) fn mqtt(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut t = MqttTrigger::new(p.required("topic")?).qos(match p.get("qos") {
            "0" => 0,
            "1" => 1,
            "2" => 2,
            _ => return None,
        });
        if !p.get("payload").is_empty() {
            t = t.payload(p.get("payload"));
        }
        t.into()
    })
}

pub(super) fn webhook(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some(
        WebhookTrigger::new(p.required("webhook_id")?)
            .local_only(match p.get("local_only") {
                "yes" => true,
                "no" => false,
                _ => return None,
            })
            .into(),
    )
}

pub(super) fn calendar(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut t = CalendarTrigger::new(
            p.required("entity_id")?,
            match p.get("event") {
                "start" => CalendarEvent::Start,
                "end" => CalendarEvent::End,
                _ => return None,
            },
        );
        if let Some(d) = p.offset()? {
            t = t.offset(d);
        }
        t.into()
    })
}

pub(super) fn tag(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some(TagTrigger::new(p.required("tag_id")?).into())
}

pub(super) fn conversation(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut commands = p
            .get("command")
            .split([';', '\n'])
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let mut t = ConversationTrigger::new(commands.next()?);
        for command in commands {
            t = t.and_command(command);
        }
        t.into()
    })
}

pub(super) fn template(p: &Properties<'_>) -> Option<ha_core_client::triggers::Trigger> {
    Some({
        let mut t = TemplateTrigger::new(p.required("value_template")?);
        if let Some(d) = p.hold()? {
            t = t.for_duration(d);
        }
        t.into()
    })
}
