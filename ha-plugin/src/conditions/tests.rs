use super::*;
use chrono::{DateTime, Utc};
use serde_json::Value;
use crate::live::Live;
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
fn state_alternatives_attribute_and_duration() {
    let live = live();
    let at = "2026-10-30T10:05:00Z";
    for (props, expected) in [
        (vec![("state", "off, on")], true),
        (vec![("state", "off")], false),
        (vec![("attribute", "mode"), ("state", "eco")], true),
        (vec![("attribute", "brightness"), ("state", "120")], true),
        (vec![("state", "on"), ("for", "00:05:00")], true),
        (vec![("state", "on"), ("for", "301")], false),
    ] {
        let mut props = props;
        props.push(("entity_id", "light.test"));
        assert_eq!(eval(Kind::State, &props, at, &live).unwrap(), expected);
    }
    assert!(
        eval(
            Kind::State,
            &[("entity_id", "light.test"), ("for", "-1")],
            at,
            &live
        )
        .is_err()
    );
}
#[test]
fn numeric_exclusive_bounds_and_attributes() {
    let live = live();
    let at = "2026-10-30T12:00:00Z";
    for (above, below, expected) in [
        ("20", "22", true),
        ("21", "22", false),
        ("20", "21", false),
        ("input_number.lower", "input_number.upper", true),
        ("", "20", false),
    ] {
        assert_eq!(
            eval(
                Kind::NumericState,
                &[
                    ("entity_id", "sensor.temp"),
                    ("above", above),
                    ("below", below)
                ],
                at,
                &live
            )
            .unwrap(),
            expected
        );
    }
    assert!(
        eval(
            Kind::NumericState,
            &[
                ("entity_id", "sensor.temp"),
                ("attribute", "temperature"),
                ("above", "22")
            ],
            at,
            &live
        )
        .unwrap()
    );
    for bound in ["NaN", "inf", "sensor.missing", "sensor.bad"] {
        assert!(
            eval(
                Kind::NumericState,
                &[("entity_id", "sensor.temp"), ("above", bound)],
                at,
                &live
            )
            .is_err()
        );
    }
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
            Kind::State,
            &[("entity_id", entity), ("state", "on")],
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
            Kind::State,
            &[("entity_id", "light.test"), ("state", "on")],
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
fn condition_properties_restore_and_selects_reject_unknown_options() {
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
            restored.set_property("before", DataValue::from("invalid"));
            assert_eq!(saved, restored.properties());
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

#[test]
fn state_condition_definition_uses_entity_specific_option_sources() {
    let node = NodeId(ticknode_shared::Uuid::nil());
    let mut cond = ConditionNode::new(Kind::State, node);
    let def = cond.definition();
    let state_prop = def.properties.iter().find(|p| p.name == "state").unwrap();
    let attr_prop = def.properties.iter().find(|p| p.name == "attribute").unwrap();
    assert_eq!(state_prop.kind, PropertyKind::Dynamic { source: "states".into(), plugin: String::new() });
    assert_eq!(attr_prop.kind, PropertyKind::Dynamic { source: "attributes".into(), plugin: String::new() });

    cond.set_property("entity_id", DataValue::from("light.kitchen"));
    let def = cond.definition();
    let state_prop = def.properties.iter().find(|p| p.name == "state").unwrap();
    let attr_prop = def.properties.iter().find(|p| p.name == "attribute").unwrap();
    assert_eq!(state_prop.kind, PropertyKind::Dynamic { source: "states:light.kitchen".into(), plugin: String::new() });
    assert_eq!(attr_prop.kind, PropertyKind::Dynamic { source: "attributes:light.kitchen".into(), plugin: String::new() });
}
