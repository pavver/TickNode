use super::*;
use super::node::strings;

fn catalog() -> Value {
    json!({
        "light": {"turn_on": {"name":"Turn on", "target":{"entity":[{"domain":"light"}]}, "fields":{
            "brightness":{"name":"Brightness", "required":true, "selector":{"number":{"min":0,"max":255}}},
            "rgb_color":{"selector":{"color_rgb":null}},
            "effect":{"selector":{"select":{"options":["none","rainbow"]}}},
            "transition":{"advanced":true,"selector":{"number":{}}}
        }}},
        "weather":{"get_forecasts":{"target":{"entity":[{"domain":["weather","sensor"]}]},"response":{"optional":false}}},
        "homeassistant":{"turn_on":{"target":{}}},
        "script":{"run_script":{}}
    })
}

#[test]
fn service_catalog_builds_options_fields_targets_and_response_support() {
    let services = services(&catalog());
    let light = &services["light.turn_on"];
    assert_eq!(
        light
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["brightness", "effect", "rgb_color", "transition"]
    );
    assert_eq!(
        light.fields[0],
        Field {
            name: "brightness".into(),
            label: "Brightness".into(),
            data_type: DataType::Number,
            required: true,
            unit: None,
        }
    );
    assert_eq!(action_source(""), "actions");
    assert_eq!(action_source("light.kitchen"), "actions:light.kitchen");
    assert!(!light.response);
    assert!(services["weather.get_forecasts"].response);
    assert_eq!(services["weather.get_forecasts"].domains, ["sensor", "weather"]);
    assert!(services["homeassistant.turn_on"].targeted);
    assert!(services["homeassistant.turn_on"].domains.is_empty());
    assert!(!services["script.run_script"].targeted);
    let light_options = options_for(&services, Some(("light.kitchen", None)));
    assert_eq!(
        light_options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
        ["light.turn_on"]
    );
    let light = &light_options[0];
    assert_eq!(
        (
            light.label.as_str(),
            light.detail.as_str(),
            light.icon.as_deref()
        ),
        ("Turn on", "light.turn_on", Some("mdi:lightbulb"))
    );
    let sensor_options = options_for(&services, Some(("sensor.temp", None)));
    assert_eq!(
        sensor_options.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
        ["weather.get_forecasts"]
    );
    assert_eq!(sensor_options[0].label, "Get forecasts");
    let targetless = options_for(&services, None);
    assert_eq!(
        targetless.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
        ["script.run_script"]
    );
    assert_eq!(target_domains("switch", &json!({"target":{}})), ["switch"]);
    assert!(!services.is_empty());
    assert!(super::services(&Value::Null).is_empty());
}

#[test]
fn entity_actions_filter_by_supported_features_and_device_class() {
    let services = services(&json!({
        "cover": {
            "open_cover": {"name": "Open cover", "target": {"entity": [{"domain": "cover", "supported_features": [1]}]}},
            "close_cover": {"name": "Close cover", "target": {"entity": [{"domain": "cover", "supported_features": [2]}]}},
            "set_cover_tilt_position": {"name": "Set tilt", "target": {"entity": [{"domain": "cover", "supported_features": [128]}]}},
            "open_shutter": {"name": "Open shutter", "target": {"entity": [{"domain": "cover", "device_class": ["shutter"]}]}}
        }
    }));
    let blind = crate::entities::test_state(
        "cover.blind",
        "open",
        json!({"supported_features": 3, "device_class": "blind"}),
    );
    let shutter = crate::entities::test_state(
        "cover.shutter",
        "closed",
        json!({"supported_features": 131, "device_class": "shutter"}),
    );
    let blind_actions: Vec<_> = options_for(&services, Some(("cover.blind", Some(&blind))))
        .into_iter()
        .map(|o| o.value)
        .collect();
    assert_eq!(blind_actions, ["cover.open_cover", "cover.close_cover"]);
    let shutter_actions: Vec<_> =
        options_for(&services, Some(("cover.shutter", Some(&shutter))))
            .into_iter()
            .map(|o| o.value)
            .collect();
    assert_eq!(
        shutter_actions,
        [
            "cover.open_cover",
            "cover.close_cover",
            "cover.open_shutter",
            "cover.set_cover_tilt_position",
        ]
    );
}

#[test]
fn actions_are_sorted_by_usage_frequency_before_alphabetical_order() {
    let services = services(&json!({
        "light": {
            "toggle": {"name": "Toggle", "target": {"entity": [{"domain": "light"}]}},
            "turn_off": {"name": "Turn off", "target": {"entity": [{"domain": "light"}]}},
            "turn_on": {"name": "Turn on", "target": {"entity": [{"domain": "light"}]}},
            "reload": {"name": "Reload", "target": {"entity": [{"domain": "light"}]}}
        },
        "cover": {
            "stop_cover": {"name": "Stop cover", "target": {"entity": [{"domain": "cover"}]}},
            "close_cover": {"name": "Close cover", "target": {"entity": [{"domain": "cover"}]}},
            "set_cover_position": {"name": "Set cover position", "target": {"entity": [{"domain": "cover"}]}},
            "open_cover": {"name": "Open cover", "target": {"entity": [{"domain": "cover"}]}},
            "toggle": {"name": "Toggle", "target": {"entity": [{"domain": "cover"}]}}
        }
    }));
    let light: Vec<_> = options_for(&services, Some(("light.kitchen", None)))
        .into_iter()
        .map(|o| o.value)
        .collect();
    assert_eq!(
        light,
        [
            "light.turn_on",
            "light.turn_off",
            "light.toggle",
            "light.reload",
        ]
    );
    let cover: Vec<_> = options_for(&services, Some(("cover.hall", None)))
        .into_iter()
        .map(|o| o.value)
        .collect();
    assert_eq!(
        cover,
        [
            "cover.open_cover",
            "cover.close_cover",
            "cover.stop_cover",
            "cover.toggle",
            "cover.set_cover_position",
        ]
    );
}

#[test]
fn selectors_map_to_pin_types() {
    for (selector, ty) in [
        ("number", DataType::Number),
        ("boolean", DataType::Bool),
        ("text", DataType::String),
        ("template", DataType::String),
        ("select", DataType::String),
        ("color_rgb", DataType::Color),
        ("time", DataType::Time),
        ("duration", DataType::Duration),
        ("object", DataType::Any),
        ("entity", DataType::Any),
    ] {
        assert_eq!(selector_type(&json!({selector:null})), ty, "{selector}");
    }
    assert_eq!(selector_type(&Value::Null), DataType::Any);
}

#[test]
fn action_snapshot_restores_the_exact_form_without_a_catalog() {
    let mut action = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    crate::with_hub(|hub, _| hub.services = services(&catalog()));
    action.set_property("target", "light.kitchen".into());
    action.set_property("action", "light.turn_on".into());
    let props = action.properties();
    assert_eq!(props[0], ("target".into(), "light.kitchen".into()));
    assert_eq!(props[1], ("action".into(), "light.turn_on".into()));
    assert_eq!(
        props.iter().find(|(k, _)| k == "fields").unwrap().1,
        strings(
            [
                "brightness:number",
                "effect:string",
                "rgb_color:color",
                "transition:number"
            ]
            .map(str::to_string)
        )
    );
    let def = action.definition();
    assert_eq!(def.properties[0].name, "target");
    assert_eq!(
        def.properties[0].kind,
        PropertyKind::Dynamic {
            source: crate::entities::ACTION_TARGETS_SOURCE.into(),
            plugin: String::new()
        }
    );
    assert_eq!(
        def.properties[1].kind,
        PropertyKind::Dynamic {
            source: "actions:light.kitchen".into(),
            plugin: String::new()
        }
    );
    assert_eq!(
        (
            def.pins[2].id,
            def.pins[2].name.as_str(),
            def.pins[2].required
        ),
        (PinId(2), "Brightness", true)
    );
    assert_eq!(
        def.pins[3].pin_type,
        ticknode_shared::PinType::Data(DataType::String)
    );
    // Switching to another entity of the same domain keeps the selected action and its fields.
    action.set_property("target", "light.bedroom".into());
    assert_eq!(action.action, "light.turn_on");
    assert_eq!(action.service.fields.len(), 4);
    // Switching to an entity of an incompatible domain resets the action while the catalog is loaded.
    action.set_property("target", "weather.home".into());
    assert!(action.action.is_empty());
    assert!(action.service.fields.is_empty());
    action.set_property("target", "light.kitchen".into());
    action.set_property("action", "light.turn_on".into());
    // Clearing the target entity resets an entity-targeted action while the catalog is loaded.
    action.set_property("target", "".into());
    assert!(action.action.is_empty());
    assert!(action.service.fields.is_empty());
    action.set_property("target", "light.kitchen".into());
    action.set_property("action", "light.turn_on".into());
    crate::with_hub(|hub, _| hub.services.clear());
    let mut restored = ActionNode::new(NodeId(ticknode_shared::Uuid::nil()));
    for (name, value) in props.clone() {
        restored.set_property(&name, value);
    }
    assert_eq!(restored.definition(), def);
    assert_eq!(restored.properties(), props);
    restored.set_property("action", "script.run_script".into());
    assert!(restored.service.fields.is_empty());
    assert_eq!(restored.definition().pins.len(), 6);
}

#[test]
fn field_values_are_ha_selector_values() {
    assert_eq!(
        field_json(&DataValue::Color(ticknode_shared::Color {
            r: 12,
            g: 34,
            b: 56,
            a: 255
        })),
        json!([12, 34, 56])
    );
    assert_eq!(
        field_json(&DataValue::Duration(std::time::Duration::from_millis(
            3_661_500
        ))),
        json!({"hours":1,"minutes":1,"seconds":1.5})
    );
    assert_eq!(
        field_json(&DataValue::Time(
            chrono::NaiveTime::from_hms_opt(7, 30, 5).unwrap()
        )),
        json!("07:30:05")
    );
    assert_eq!(field_json(&DataValue::from(120i64)), json!(120));
    assert_eq!(field_json(&DataValue::Bool(false)), json!(false));
    assert_eq!(field_json(&DataValue::from("rainbow")), json!("rainbow"));
}

#[test]
fn response_is_emitted_only_for_response_data() {
    let out = outputs(Ok(
        json!({"context":{},"response":{"weather.home":{"forecast":[]}}}),
    ));
    assert!(
        out.iter()
            .any(|o| matches!(o,Output::Data(p,DataValue::Object{..}) if *p == RESPONSE))
    );
    for value in [Value::Null, json!({"response":null}), json!({"context":{}})] {
        assert!(
            !outputs(Ok(value))
                .iter()
                .any(|o| matches!(o,Output::Data(p,_) if *p == RESPONSE))
        );
    }
    assert!(
        outputs(Err("rejected".into()))
            .iter()
            .any(|o| matches!(o,Output::Exec(p) if *p == FAILED))
    );
}
