use super::*;
use crate::{entities::test_state, live::Live};

fn check(domain: &str, service: &str, state: &str, attributes: Value, data: Value) -> bool {
    let entity = format!("{domain}.test");
    let mut live = Live::default();
    live.snapshot(&[test_state(&entity, state, attributes)]);
    unchanged(&live, domain, service, &entity, &data)
}

#[test]
fn state_effects_require_the_exact_state_and_no_extra_fields() {
    for domain in [
        "switch",
        "light",
        "fan",
        "input_boolean",
        "siren",
        "humidifier",
        "automation",
        "remote",
        "climate",
        "lock",
        "cover",
    ] {
        let (services, states) = match domain {
            "lock" => (["lock", "unlock"], ["locked", "unlocked"]),
            "cover" => (["open_cover", "close_cover"], ["open", "closed"]),
            _ => (["turn_on", "turn_off"], ["on", "off"]),
        };
        for (index, service) in services.iter().enumerate() {
            assert!(
                check(domain, service, states[index], json!({}), json!({})),
                "{domain}.{service}"
            );
            for state in [
                states[1 - index],
                "unknown",
                "unavailable",
                "heat",
                "opening",
            ] {
                assert!(
                    !check(domain, service, state, json!({}), json!({})),
                    "{domain}/{state}"
                );
            }
            for data in [
                json!({"brightness":42}),
                json!({"rgb_color":[1,2,3]}),
                json!({"percentage":50}),
                json!({"transition":1}),
                json!({"brightness":null}),
            ] {
                assert!(!check(domain, service, states[index], json!({}), data));
            }
        }
    }
    assert!(!check("light", "toggle", "on", json!({}), json!({})));
    assert!(!check("sensor", "turn_on", "on", json!({}), json!({})));
}

#[test]
fn numeric_effects_compare_finite_values_with_a_small_tolerance() {
    for domain in ["number", "input_number"] {
        for actual in ["1.5", "1.500000001", "1.5000005"] {
            assert!(check(
                domain,
                "set_value",
                actual,
                json!({"unit_of_measurement":"kW"}),
                json!({"value":1.5})
            ));
        }
        for actual in ["1.51", "1500", "NaN", "inf", "unknown", "unavailable"] {
            assert!(!check(
                domain,
                "set_value",
                actual,
                json!({}),
                json!({"value":1.5})
            ));
        }
        for data in [
            json!({}),
            json!({"value":"1.5"}),
            json!({"value":null}),
            json!({"value":1.5,"extra":1}),
        ] {
            assert!(!check(domain, "set_value", "1.5", json!({}), data));
        }
    }
}

#[test]
fn text_effects_require_an_exact_string_and_only_the_recognized_field() {
    for (domain, service, field) in [
        ("select", "select_option", "option"),
        ("input_select", "select_option", "option"),
        ("text", "set_value", "value"),
        ("input_text", "set_value", "value"),
    ] {
        assert!(check(
            domain,
            service,
            "Ready",
            json!({}),
            json!({field:"Ready"})
        ));
        assert!(check(domain, service, "", json!({}), json!({field:""})));
        for state in ["ready", "other", "unknown", "unavailable"] {
            assert!(!check(
                domain,
                service,
                state,
                json!({}),
                json!({field:"Ready"})
            ));
            if !matches!(state, "unknown" | "unavailable") {
                assert!(check(
                    domain,
                    service,
                    state,
                    json!({}),
                    json!({field:state})
                ));
            } else {
                assert!(!check(
                    domain,
                    service,
                    state,
                    json!({}),
                    json!({field:state})
                ));
            }
        }
        for data in [
            json!({field:"ready"}),
            json!({field:1}),
            json!({}),
            json!({field:"Ready","extra":true}),
        ] {
            assert!(!check(domain, service, "Ready", json!({}), data));
        }
    }
}

#[test]
fn climate_temperature_compares_the_setpoint_and_rejects_other_effects() {
    assert!(check(
        "climate",
        "set_temperature",
        "heat",
        json!({"temperature":68.0,"current_temperature":60.0}),
        json!({"temperature":68.00000001})
    ));
    for attrs in [
        json!({}),
        json!({"temperature":67}),
        json!({"temperature":"68"}),
        json!({"current_temperature":68}),
    ] {
        assert!(!check(
            "climate",
            "set_temperature",
            "heat",
            attrs,
            json!({"temperature":68})
        ));
    }
    for data in [
        json!({"temperature":68,"hvac_mode":"cool"}),
        json!({"target_temp_low":68}),
        json!({}),
    ] {
        assert!(!check(
            "climate",
            "set_temperature",
            "heat",
            json!({"temperature":68}),
            data
        ));
    }
    for state in ["unknown", "unavailable"] {
        assert!(!check(
            "climate",
            "set_temperature",
            state,
            json!({"temperature":68}),
            json!({"temperature":68})
        ));
    }
}

#[test]
fn all_explicit_targets_must_be_known_ready_and_already_set() {
    let mut live = Live::default();
    let states = [
        test_state("light.a", "on", json!({})),
        test_state("light.b", "on", json!({})),
    ];
    live.snapshot(&states);
    assert!(unchanged(
        &live,
        "light",
        "turn_on",
        " light.a, light.b ",
        &json!({})
    ));
    for target in [
        "",
        "all",
        "area:kitchen",
        "device:123",
        "label:night",
        "light.a,",
        "light.a,light.missing",
        "light.a,switch.b",
        r#"{"area_id":"kitchen"}"#,
    ] {
        assert!(
            !unchanged(&live, "light", "turn_on", target, &json!({})),
            "{target}"
        );
    }
    for key in ["area_id", "device_id", "label_id", "entity_id"] {
        assert!(!unchanged(
            &live,
            "light",
            "turn_on",
            "light.a",
            &json!({key:"other"})
        ));
    }
    for state in ["off", "unknown", "unavailable"] {
        live.snapshot(&[states[0].clone(), test_state("light.b", state, json!({}))]);
        assert!(!unchanged(
            &live,
            "light",
            "turn_on",
            "light.a,light.b",
            &json!({})
        ));
    }
    live.reset();
    assert!(!unchanged(&live, "light", "turn_on", "light.a", &json!({})));
}

#[test]
fn synthetic_actions_map_both_booleans_without_leaking_state_into_service_data() {
    for effect in super::effects::STATE_ACTIONS {
        for state in [false, true] {
            let mut data = json!({"state":state});
            assert_eq!(
                resolve_state(effect.domain, "set_state", &mut data).unwrap(),
                effect.services[usize::from(state)]
            );
            assert_eq!(data, json!({}));
        }
        for mut data in [json!({}), json!({"state":1}), json!({"state":"on"})] {
            assert!(resolve_state(effect.domain, "set_state", &mut data).is_err());
        }
    }
    assert!(super::effects::state_service("climate").is_none());
}

#[test]
fn legacy_skip_default_and_explicit_preferences_round_trip() {
    let mut node = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    let preference = |node: &ActionNode| {
        node.properties()
            .into_iter()
            .find(|(k, _)| k == "skip_unchanged")
            .unwrap()
            .1
    };
    assert_eq!(preference(&node), "yes".into());
    node.set_property("skip_unchanged", "no".into());
    let properties = node.properties();
    let mut restored = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    for (name, value) in properties {
        restored.set_property(&name, value);
    }
    assert_eq!(preference(&restored), "no".into());
    restored.set_property("skip_unchanged", "invalid".into());
    assert_eq!(preference(&restored), "no".into());
    restored.set_property("skip_unchanged", "yes".into());
    assert_eq!(preference(&restored), "yes".into());
}
