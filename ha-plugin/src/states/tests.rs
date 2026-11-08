use super::*;
use serde_json::json;

#[test]
fn state_options_for_light_include_on_off_and_current() {
    let state = crate::entities::test_state("light.hall", "on", json!({}));
    let options = state_options("light.hall", Some(&state), true);
    assert_eq!(options[0].value, "");
    assert_eq!(options[1].value, "on");
    assert!(options[1].label.contains("Current state"));
    assert!(options.iter().any(|o| o.value == "off"));
    assert!(options.iter().any(|o| o.value == "unavailable"));
}

#[test]
fn state_options_for_door_use_open_closed_labels() {
    let state = crate::entities::test_state(
        "binary_sensor.front_door",
        "off",
        json!({"device_class": "door"}),
    );
    let options = state_options("binary_sensor.front_door", Some(&state), false);
    assert_eq!(options[0].value, "off");
    assert!(options.iter().any(|o| o.value == "on" && o.label.contains("Open")));
}

#[test]
fn attribute_options_list_attributes_with_values() {
    let state = crate::entities::test_state(
        "light.hall",
        "on",
        json!({"brightness": 255, "friendly_name": "Hall light"}),
    );
    let options = attribute_options("light.hall", Some(&state));
    assert_eq!(options[0].value, "");
    assert!(options.iter().any(|o| o.value == "brightness" && o.detail.contains("255")));
    assert!(options.iter().any(|o| o.value == "friendly_name"));
}

#[test]
fn picker_sources_trim_ids_and_keep_global_sources_for_blank_ids() {
    for (entity, state, attr) in [
        ("", "states", "attributes"),
        (" \n", "states", "attributes"),
        (" light.hall ", "states:light.hall", "attributes:light.hall"),
    ] {
        assert_eq!(state_source(entity), state);
        assert_eq!(attribute_source(entity), attr);
    }
    assert_eq!(
        crate::actions::action_source(" light.hall "),
        "actions:light.hall"
    );
    assert_eq!(crate::actions::action_source(" \n"), "actions");
}

#[test]
fn attribute_details_keep_strings_and_json_scalars_and_containers() {
    let state = crate::entities::test_state("sensor.demo","1",json!({"word":"hello","flag":true,"list":["a",2],"object":{"n":1}}));
    let options = attribute_options("sensor.demo",Some(&state));
    for (key,detail) in [("word","Value: hello"),("flag","Value: true"),("list","Value: [\"a\",2]"),("object","Value: {\"n\":1}")] {
        assert_eq!(options.iter().find(|option|option.value==key).unwrap().detail,detail);
    }
    assert_eq!(attribute_options("",None).len(),1);
}
