use super::*;
use crate::conditions::{ConditionNode, Kind};
use crate::triggers::{Platform, TriggerNode};

#[test]
fn trigger_validation_covers_boundaries_and_preserves_legacy_text() {
    for (platform, name, valid, invalid) in [
        (
            Platform::Sun,
            "offset",
            vec!["", "-30m", "00:30:00", "+2h"],
            vec!["soon", "--1m", "1t", "18446744073709551615s"],
        ),
        (
            Platform::Calendar,
            "offset",
            vec!["-00:30:00"],
            vec!["00:60:00"],
        ),
        (
            Platform::Template,
            "for",
            vec!["0", "1h 30m", "00:30:00"],
            vec!["-30m", "2t"],
        ),
        (
            Platform::Event,
            "event_data",
            vec!["", "{}", "{\"entity_id\":\"light.hall\"}"],
            vec!["[]", "null", "1", "{bad}"],
        ),
        (
            Platform::Event,
            "event_type",
            vec!["", "state_changed"],
            vec![" ", "state changed", "event\n"],
        ),
        (
            Platform::Mqtt,
            "topic",
            vec!["", "#", "+", "home/+/state", "home//#", "/"],
            vec!["a/#/b", "a+", "a/#x", "a/+b", "\0"],
        ),
        (
            Platform::Webhook,
            "webhook_id",
            vec!["", "Ab_01-x"],
            vec!["a/b", "a.b", "test id", "тест"],
        ),
        (
            Platform::TimePattern,
            "hours",
            vec!["", "0", "23", "*", "/1", "/23"],
            vec!["24", "/0", "/24", "-1", "+1", "1.5"],
        ),
        (
            Platform::TimePattern,
            "minutes",
            vec!["59", "/59"],
            vec!["60", "/60"],
        ),
        (
            Platform::TimePattern,
            "seconds",
            vec!["59", "/59"],
            vec!["60", "*/5"],
        ),
        (
            Platform::Time,
            "at",
            vec![
                "",
                "07:30",
                "23:59:59",
                "input_datetime.alarm",
                "sensor.alarm",
            ],
            vec!["24:00", "input_datetime.", "a.b.c", "sensor.bad id"],
        ),
    ] {
        let mut node = TriggerNode::new(platform, NodeId(ticknode_shared::Uuid::nil()));
        for value in valid {
            assert!(
                node.validate_property(name, &value.into()).is_ok(),
                "{platform:?}.{name}: {value}"
            );
            node.set_property(name, value.into());
            assert!(node.properties().contains(&(name.into(), value.into())));
        }
        for value in invalid {
            let message = node.validate_property(name, &value.into()).unwrap_err();
            assert!(message.key.starts_with("home-assistant-error-"));
            for (_, source) in crate::HomeAssistant::TRANSLATIONS {
                assert!(
                    source.contains(&format!("{} =", message.key)),
                    "{}",
                    message.key
                );
            }
            node.set_property(name, value.into());
            assert!(
                stored_trigger(
                    platform,
                    &node
                        .properties()
                        .into_iter()
                        .map(|(n, v)| match v {
                            DataValue::String(s) => (n, s),
                            _ => unreachable!(),
                        })
                        .collect()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn typed_schemas_and_conditional_solar_offsets_keep_stored_properties() {
    let mut node = ConditionNode::new(Kind::Sun, NodeId(ticknode_shared::Uuid::nil()));
    let kind = |node: &ConditionNode, name: &str| {
        node.definition()
            .properties
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
            .kind
    };
    assert_eq!(kind(&node, "after_offset"), PropertyKind::Hidden);
    node.set_property("after_offset", "-30m".into());
    node.set_property("after", "sunset".into());
    assert_eq!(
        kind(&node, "after_offset"),
        PropertyKind::Duration { signed: true }
    );
    assert_eq!(kind(&node, "before_offset"), PropertyKind::Hidden);
    node.set_property("after", "none".into());
    assert!(
        node.properties()
            .contains(&("after_offset".into(), "-30m".into()))
    );
    node.set_property("before_offset", "bad".into());
    assert!(!node.polls(), "disabled offsets do not prevent evaluation");
    node.set_property("before", "sunrise".into());
    assert!(
        node.polls(),
        "legacy invalid values get runtime error polling"
    );
    node.set_property("before_offset", "00:30:00".into());
    assert!(!node.polls());
    let time = ConditionNode::new(Kind::Time, NodeId(ticknode_shared::Uuid::nil()));
    assert!(time.validate_property("weekday", &"mon,tue".into()).is_ok());
    assert!(time.validate_property("weekday", &"monday".into()).is_err());
    assert!(time.validate_property("after", &"24:00".into()).is_err());
    assert_eq!(kind(&time, "after"), PropertyKind::Time);
    assert!(matches!(kind(&time, "weekday"), PropertyKind::MultiSelect(days) if days.len() == 7));
    for (platform, name, kind) in [
        (
            Platform::Sun,
            "offset",
            PropertyKind::Duration { signed: true },
        ),
        (
            Platform::Calendar,
            "offset",
            PropertyKind::Duration { signed: true },
        ),
        (
            Platform::Template,
            "for",
            PropertyKind::Duration { signed: false },
        ),
        (
            Platform::Template,
            "value_template",
            PropertyKind::Multiline,
        ),
        (Platform::Conversation, "command", PropertyKind::Multiline),
        (Platform::Event, "event_data", PropertyKind::Multiline),
        (Platform::Mqtt, "payload", PropertyKind::Text),
    ] {
        assert_eq!(
            platform
                .properties()
                .into_iter()
                .find(|p| p.name == name)
                .unwrap()
                .kind,
            kind
        );
    }
}
