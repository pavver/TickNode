use super::*;
use serde_json::json;

fn props(platform: Platform, values: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut props = platform.defaults();
    props.extend(values.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    props
}

fn wire(platform: Platform, values: &[(&str, &str)]) -> Value {
    serde_json::to_value(build(platform, &props(platform, values)).unwrap().expect("complete config").trigger).unwrap()
}

#[test]
fn every_platform_builds_the_expected_typed_trigger() {
    let cases: &[(Platform, &[(&str, &str)], Value)] = &[
        (
            Platform::State,
            &[("entity_id", "light.hall"), ("attribute", "brightness"), ("from", "0"), ("to", "255"), ("for", "1h 30m")],
            json!({"trigger":"state", "entity_id":["light.hall"], "attribute":"brightness", "from":"0", "to":"255", "for":"01:30:00"}),
        ),
        (
            Platform::NumericState,
            &[
                ("entity_id", "sensor.t"),
                ("above", "21.5"),
                ("below", "input_number.max"),
                ("attribute", "temperature"),
                ("value_template", "{{ state.state | float }}"),
                ("for", "00:05:00"),
            ],
            json!({"trigger":"numeric_state", "entity_id":["sensor.t"], "above":21.5, "below":"input_number.max", "attribute":"temperature", "value_template":"{{ state.state | float }}", "for":"00:05:00"}),
        ),
        (Platform::Time, &[("at", "07:30")], json!({"trigger":"time", "at":["07:30"]})),
        (
            Platform::TimePattern,
            &[("hours", "*"), ("minutes", "/5"), ("seconds", "0")],
            json!({"trigger":"time_pattern", "hours":"*", "minutes":"/5", "seconds":"0"}),
        ),
        (Platform::Sun, &[("event", "sunset"), ("offset", "-00:30:00")], json!({"trigger":"sun", "event":"sunset", "offset":"-00:30:00"})),
        (
            Platform::Zone,
            &[("entity_id", "person.me"), ("zone", "zone.home"), ("event", "leave")],
            json!({"trigger":"zone", "entity_id":"person.me", "zone":"zone.home", "event":"leave"}),
        ),
        (
            Platform::Event,
            &[("event_type", "my_event"), ("event_data", r#"{"kind":"door","n":1}"#)],
            json!({"trigger":"event", "event_type":["my_event"], "event_data":{"kind":"door", "n":1}}),
        ),
        (Platform::HomeAssistant, &[("event", "shutdown")], json!({"trigger":"homeassistant", "event":"shutdown"})),
        (
            Platform::Mqtt,
            &[("topic", "home/+/button"), ("payload", "on"), ("qos", "2")],
            json!({"trigger":"mqtt", "topic":"home/+/button", "payload":"on", "qos":2}),
        ),
        (
            Platform::Webhook,
            &[("webhook_id", "secret"), ("local_only", "no")],
            json!({"trigger":"webhook", "webhook_id":"secret", "local_only":false}),
        ),
        (
            Platform::Calendar,
            &[("entity_id", "calendar.work"), ("event", "end"), ("offset", "-15m")],
            json!({"trigger":"calendar", "entity_id":"calendar.work", "event":"end", "offset":"-00:15:00"}),
        ),
        (Platform::Tag, &[("tag_id", "123-456")], json!({"trigger":"tag", "tag_id":["123-456"]})),
        (
            Platform::Conversation,
            &[("command", " hello ; goodbye\n [it's ]party time ; ")],
            json!({"trigger":"conversation", "command":["hello", "goodbye", "[it's ]party time"]}),
        ),
        (
            Platform::Template,
            &[("value_template", "{{ true }}"), ("for", "90")],
            json!({"trigger":"template", "value_template":"{{ true }}", "for":"00:01:30"}),
        ),
    ];
    assert_eq!(cases.len(), PLATFORMS.len());
    for (platform, values, expected) in cases {
        assert_eq!(wire(*platform, values), *expected, "{platform:?}");
    }
    // Threshold text belongs to HA even if it is not a syntactically valid entity id.
    assert_eq!(wire(Platform::NumericState, &[("entity_id", "sensor.t"), ("above", "not an entity")])["above"], "not an entity");
    assert_eq!(wire(Platform::NumericState, &[("entity_id", "sensor.t"), ("above", "sensor.limit"), ("below", "30")])["below"], 30.0);
    for at in ["07:30:05", "input_datetime.alarm", "sensor.next_alarm"] {
        assert_eq!(wire(Platform::Time, &[("at", at)])["at"], json!([at]));
    }
    assert_eq!(wire(Platform::Webhook, &[("webhook_id", "id")])["local_only"], true);
    assert!(wire(Platform::State, &[("entity_id", "light.x")]).get("to").is_none());
}

#[test]
fn incomplete_and_invalid_properties_do_not_subscribe() {
    for platform in PLATFORMS {
        let defaults = platform.defaults();
        let config = build(*platform, &defaults).unwrap();
        assert_eq!(config.is_some(), matches!(platform, Platform::Sun | Platform::HomeAssistant), "{platform:?}");
        // Even platforms with complete defaults require valid select values.
        assert!(build(*platform, &BTreeMap::new()).unwrap().is_none(), "{platform:?}");
    }
    for (platform, values) in [
        (Platform::State, vec![("entity_id", "light.x"), ("for", "-")]),
        (Platform::NumericState, vec![("entity_id", "sensor.t")]),
        (Platform::NumericState, vec![("entity_id", "sensor.t"), ("above", "NaN")]),
        (Platform::Time, vec![("at", "25:99")]),
        (Platform::TimePattern, vec![("minutes", "/0")]),
        (Platform::TimePattern, vec![("hours", "24")]),
        (Platform::Sun, vec![("offset", "bad")]),
        (Platform::Zone, vec![("entity_id", "person.me")]),
        (Platform::Event, vec![("event_data", "{}")]),
        (Platform::HomeAssistant, vec![("event", "bad")]),
        (Platform::Mqtt, vec![("topic", "t"), ("qos", "3")]),
        (Platform::Webhook, vec![("webhook_id", "x"), ("local_only", "bad")]),
        (Platform::Calendar, vec![("entity_id", "calendar.x"), ("offset", "bad")]),
        (Platform::Tag, vec![("tag_id", "")]),
        (Platform::Conversation, vec![("command", ";\n ;")]),
        (Platform::Template, vec![("value_template", "{{ true }}"), ("for", "-1")]),
    ] {
        assert!(build(platform, &props(platform, &values)).unwrap().is_none(), "{platform:?}");
    }
    for bad in ["{", "null", "[]", "42", r#""text""#] {
        assert!(build(Platform::Event, &props(Platform::Event, &[("event_type", "test"), ("event_data", bad)])).is_err());
    }
}

#[test]
fn duration_formats_signs_and_overflow() {
    for (text, seconds) in [("90", 90.0), ("1h 30m", 5400.0), ("00:05:00", 300.0), ("27:00:00.5", 97200.5), ("500ms", 0.5)] {
        assert_eq!(duration(text).unwrap().as_secs_f64(), seconds);
    }
    for bad in ["-1", "00:60:00", "00:00:60", "-00:30:00", "NaN", "inf", "1e100", "99999999999999999999:00:00", "00:01", "", ":00:00"] {
        assert!(duration(bad).is_none(), "{bad}");
    }
    assert_eq!(signed_duration("-00:30:00"), Some(TimeDelta::minutes(-30)));
    assert_eq!(signed_duration("+1h 30m"), Some(TimeDelta::minutes(90)));
    assert_eq!(signed_duration("-90"), Some(TimeDelta::seconds(-90)));
}

fn sample(platform: Platform, raw: Value, attribute: &str) -> Vec<Output> {
    let config = TriggerConfig { platform, trigger: StateTrigger::new("sensor.t").into(), attribute: attribute.into() };
    outputs(&config, &TriggerFired { trigger: raw, context: None })
}

fn data(outputs: &[Output], pin: u32) -> DataValue {
    outputs
        .iter()
        .find_map(|o| match o {
            Output::Data(p, v) if p.0 == pin => Some(v.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn state_numeric_and_zone_outputs_keep_states_and_full_info() {
    let state = serde_json::to_value(crate::entities::test_state("sensor.t", "23.5", json!({"temperature":"24.5"}))).unwrap();
    // State serializes attributes with an external tag; HA sends a plain map.
    let mut state = state;
    state["attributes"] = json!({"temperature":"24.5"});
    let raw = json!({"platform":"numeric_state", "entity_id":"sensor.t", "from_state":null, "to_state":state, "extra":{"a":true}, "description":"test"});
    for platform in [Platform::State, Platform::NumericState, Platform::Zone] {
        let outputs = sample(platform, raw.clone(), "");
        assert_eq!(outputs.last(), Some(&Output::Exec(TRIGGERED)));
        let info = data(&outputs, 1);
        assert_eq!(field_string(&info, "platform"), Some("numeric_state"));
        assert_eq!(crate::convert::value_to_json(field(&info, "extra").unwrap()), json!({"a":true}));
        assert_eq!(data(&outputs, 2), DataValue::from("sensor.t"));
        assert!(outputs.contains(&Output::Clear(PinId(3))));
        assert_eq!(field_string(&data(&outputs, 4), "state"), Some("23.5"));
        if platform == Platform::NumericState {
            assert_eq!(data(&outputs, 5), DataValue::from(23.5));
        }
    }
    let outputs = sample(Platform::NumericState, raw.clone(), "temperature");
    assert_eq!(data(&outputs, 5), DataValue::from(24.5));
    for attribute in ["missing", "temperature"] {
        let mut raw = raw.clone();
        raw["to_state"]["attributes"]["temperature"] = json!("unknown");
        assert!(sample(Platform::NumericState, raw, attribute).contains(&Output::Clear(PinId(5))));
    }
}

#[test]
fn platform_outputs_extract_real_trigger_variable_shapes() {
    for (platform, raw, expected) in [
        (Platform::Event, json!({"event":{"event_type":"button", "data":{"id":7}}}), vec![json!({"id":7}), json!("button")]),
        (Platform::Mqtt, json!({"topic":"a/b", "payload":"on"}), vec![json!("a/b"), json!("on")]),
        (Platform::Webhook, json!({"json":{"a":1}, "data":{}, "query":{"q":"x"}}), vec![json!({"a":1}), json!({"q":"x"})]),
        (Platform::Webhook, json!({"data":{"form":"yes"}}), vec![json!({"form":"yes"}), json!({})]),
        (Platform::Webhook, json!({"json":[1,2]}), vec![json!({"value":[1,2]}), json!({})]),
        (Platform::Calendar, json!({"calendar_event":{"summary":"Meeting"}}), vec![json!({"summary":"Meeting"})]),
        (
            Platform::Tag,
            json!({"event":{"event_type":"tag_scanned","data":{"tag_id":"tag", "device_id":"phone"}}}),
            vec![json!("tag"), json!("phone")],
        ),
        (Platform::Conversation, json!({"sentence":"hello", "slots":{"room":"hall"}}), vec![json!("hello"), json!({"room":"hall"})]),
        (Platform::Time, json!({"description":"time"}), vec![json!("time")]),
        (Platform::TimePattern, json!({"description":"pattern"}), vec![json!("pattern")]),
        (Platform::Sun, json!({"description":"sunrise"}), vec![json!("sunrise")]),
        (Platform::HomeAssistant, json!({"description":"start"}), vec![json!("start")]),
        (Platform::Template, json!({"description":"true"}), vec![json!("true")]),
    ] {
        let outputs = sample(platform, raw, "");
        assert_eq!(outputs.last(), Some(&Output::Exec(TRIGGERED)));
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(crate::convert::value_to_json(&data(&outputs, i as u32 + 2)), *expected, "{platform:?}");
        }
    }
}

#[test]
fn trigger_properties_round_trip_and_all_texts_are_translated() {
    let ftl = crate::HomeAssistant::TRANSLATIONS[0].1;
    let key_part = |s: &str| {
        if s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { s.to_string() } else { ticknode_shared::fluent_slug(s) }
    };
    for platform in PLATFORMS {
        let mut node = TriggerNode::new(*platform, NodeId(ticknode_shared::Uuid::nil()));
        let properties = node.properties();
        for (name, value) in &properties {
            node.set_property(name, value.clone());
        }
        assert_eq!(node.properties(), properties);
        let def = node.definition();
        let prefix = format!("node-{}", ticknode_shared::fluent_slug(&def.type_name));
        let mut keys = vec![format!("{prefix}-title"), format!("{prefix}-description"), format!("{prefix}-help")];
        keys.extend(def.pins.iter().map(|p| format!("{prefix}-pin-{}", key_part(&p.name))));
        for property in def.properties {
            let value = match &property.kind {
                PropertyKind::Select(options) => options.last().unwrap().0.as_str(),
                _ => "custom value",
            };
            node.set_property(&property.name, DataValue::from(value));
            keys.extend([
                format!("{prefix}-prop-{}-title", key_part(&property.name)),
                format!("{prefix}-prop-{}-description", key_part(&property.name)),
            ]);
            if let PropertyKind::Select(options) = property.kind {
                keys.extend(
                    options
                        .iter()
                        .map(|(v, _)| format!("{prefix}-prop-{}-option-{}", key_part(&property.name), ticknode_shared::fluent_slug(v))),
                );
            }
        }
        let mut restored = TriggerNode::new(*platform, NodeId(ticknode_shared::Uuid::nil()));
        for (key, value) in node.properties() {
            restored.set_property(&key, value);
        }
        assert_eq!(restored.properties(), node.properties());
        for key in keys {
            assert!(ftl.contains(&format!("{key} =")), "missing {key}");
        }
    }
}
