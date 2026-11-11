use super::*;
use crate::live::Live;
use chrono::{DateTime, Utc};
use serde_json::Value;
use serde_json::json;

fn now(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}
fn state(entity: &str, value: &str, attributes: Value) -> ha_core_client::State {
    serde_json::from_value(
        json!({"entity_id":entity,"state":value,"attributes":attributes,
        "last_changed":"2026-10-30T10:00:00Z", "last_updated":"2026-10-30T10:00:00Z"}),
    )
    .unwrap()
}
fn live() -> Live {
    let mut live = Live::default();
    live.snapshot(&[
        state("light.test", "on", json!({"brightness":120,"mode":"eco"})),
        state("sensor.temp", "21", json!({"temperature":23})),
        state("input_number.lower", "20", json!({})),
        state("input_number.upper", "22", json!({})),
        state("person.me", "home", json!({})),
        state("zone.home", "1", json!({"friendly_name":"Home"})),
        state("zone.work", "0", json!({"friendly_name":"Office"})),
        state(
            "sun.sun",
            "above_horizon",
            json!({"next_rising":"2026-10-31T05:00:00Z", "next_setting":"2026-10-30T16:00:00Z"}),
        ),
        state("sensor.bad", "unavailable", json!({})),
    ]);
    live.test_offset(7200);
    live
}
fn eval(kind: Kind, properties: &[(&str, &str)], at: &str, live: &Live) -> Result<bool, String> {
    evaluate(
        kind,
        &properties
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        live,
        now(at),
    )
}

#[test]
fn local_time_windows_midnight_weekdays_and_boundaries() {
    let live = live();
    let props = [
        ("after", "22:00"),
        ("before", "06:00"),
        ("weekday", "fri,sat"),
    ];
    for (at, expected) in [
        ("2026-10-30T20:00:00Z", true),
        ("2026-10-30T23:00:00Z", true),
        ("2026-10-31T04:00:00Z", false),
        ("2026-11-01T01:00:00Z", false),
    ] {
        assert_eq!(eval(Kind::Time, &props, at, &live).unwrap(), expected);
    }
    assert!(
        eval(
            Kind::Time,
            &[("after", "10:00"), ("before", "14:00")],
            "2026-10-30T10:00:00Z",
            &live
        )
        .unwrap()
    );
    assert!(
        !eval(
            Kind::Time,
            &[("before", "12:00:00")],
            "2026-10-30T10:00:00Z",
            &live
        )
        .unwrap()
    );
    assert!(
        eval(
            Kind::Time,
            &[("weekday", "bad")],
            "2026-10-30T10:00:00Z",
            &live
        )
        .is_err()
    );
}
#[test]
fn sun_boundaries_and_signed_offsets() {
    let live = live();
    let p = [
        ("after", "sunrise"),
        ("after_offset", "30m"),
        ("before", "sunset"),
        ("before_offset", "-00:30:00"),
    ];
    for (at, expected) in [
        ("2026-10-30T05:29:59Z", false),
        ("2026-10-30T05:30:00Z", true),
        ("2026-10-30T15:30:00Z", true),
        ("2026-10-30T15:30:01Z", false),
    ] {
        assert_eq!(eval(Kind::Sun, &p, at, &live).unwrap(), expected);
    }
    assert!(
        !eval(
            Kind::Sun,
            &[("after", "sunset"), ("before", "sunrise")],
            "2026-10-30T20:00:00Z",
            &live
        )
        .unwrap()
    );
}
#[test]
fn zone_home_friendly_name_and_not_home() {
    let mut live = live();
    let at = "2026-10-30T10:00:00Z";
    let props = [("entity_id", "person.me"), ("zone", "zone.home")];
    assert!(eval(Kind::Zone, &props, at, &live).unwrap());
    assert!(
        !eval(
            Kind::Zone,
            &[("entity_id", "person.me"), ("zone", "zone.work")],
            at,
            &live
        )
        .unwrap()
    );
    live.snapshot(&[
        state("device_tracker.me", "Office", json!({})),
        state("zone.work", "0", json!({"friendly_name":"Office"})),
    ]);
    assert!(
        eval(
            Kind::Zone,
            &[("entity_id", "device_tracker.me"), ("zone", "zone.work")],
            at,
            &live
        )
        .unwrap()
    );
}
#[test]
fn missing_unavailable_and_disconnected_fail_closed() {
    let mut live = live();
    for entity in ["sensor.missing", "sensor.bad"] {
        let result = eval(
            Kind::Zone,
            &[("entity_id", entity), ("zone", "zone.home")],
            "2026-10-30T10:00:00Z",
            &live,
        );
        assert!(result.is_err());
        assert!(matches!(
            &outputs(result.unwrap_or(false))[0],
            Output::Data(RESULT, DataValue::Bool(false))
        ));
        assert!(matches!(&outputs(false)[1], Output::Exec(NO)));
    }
    live.reset();
    assert!(
        eval(
            Kind::Zone,
            &[("entity_id", "light.test"), ("zone", "zone.home")],
            "2026-10-30T10:00:00Z",
            &live
        )
        .is_err()
    );
    assert!(eval(Kind::Time, &[], "2026-10-30T10:00:00Z", &live).is_err());
}
#[test]
fn template_truth_values() {
    for v in [json!(true), json!("True"), json!("true"), json!("TRUE")] {
        assert!(template_bool(&v));
    }
    for v in [
        json!(false),
        json!("false"),
        json!(0),
        json!(1),
        json!("1"),
        json!("yes"),
        json!("on"),
        json!("enable"),
        json!(" true "),
        json!(null),
    ] {
        assert!(!template_bool(&v));
    }
}

#[test]
fn condition_properties_restore_and_interactive_selects_reject_unknown_options() {
    let node = NodeId(ticknode_shared::Uuid::nil());
    for kind in KINDS {
        let mut original = ConditionNode::new(*kind, node);
        for property in kind.definition().properties {
            let value = if let PropertyKind::Select(options) = property.kind {
                options.last().unwrap().0.clone()
            } else {
                format!("value for {}", property.name)
            };
            original.set_property(&property.name, DataValue::from(value.as_str()));
        }
        let mut restored = ConditionNode::new(*kind, node);
        for (key, value) in original.properties() {
            restored.set_property(&key, value);
        }
        assert_eq!(original.properties(), restored.properties());
        let saved = restored.properties();
        if *kind == Kind::Sun {
            assert!(restored.validate_property("before", &DataValue::from("invalid")).is_err());
            assert_eq!(saved, restored.properties());
            restored.set_property("before", "invalid".into());
            assert!(restored.polls(), "legacy invalid options stay visible as runtime errors");
        }
    }
}

#[test]
fn equal_time_boundaries_cover_a_full_day() {
    let live = live();
    for at in [
        "2026-10-30T00:00:00Z",
        "2026-10-30T10:00:00Z",
        "2026-10-30T22:00:00Z",
    ] {
        assert!(
            eval(
                Kind::Time,
                &[("after", "12:00"), ("before", "12:00")],
                at,
                &live
            )
            .unwrap()
        );
    }
}
