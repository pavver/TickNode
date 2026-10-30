//! The entity picker options: one per Home Assistant entity, built from a `get_states` answer.

use ha_core_client::State;
use ticknode_plugin_sdk::PropertyOption;

use crate::icons::entity_icon;

/// Name of the option source used by the `entity_id` property of `ha/state_changed`.
pub const ENTITIES_SOURCE: &str = "entities";

/// Option of one entity: value and detail are the entity id, the label is its `friendly_name`
/// (the entity id when there is none), the icon is the `icon` attribute or the frontend's default.
pub fn entity_option(state: &State) -> PropertyOption {
    let id = &state.entity_id;
    let label = state.attributes.friendly_name().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(id);
    let device_class: Option<String> = state.attributes.get_as("device_class");
    let icon = entity_icon(id, &state.state, device_class.as_deref(), state.attributes.icon());
    PropertyOption { value: id.clone(), label: label.to_string(), detail: id.clone(), icon: Some(icon) }
}

/// Options of all entities, sorted by label (case-insensitive), then by entity id.
pub fn entity_options(states: &[State]) -> Vec<PropertyOption> {
    let mut options: Vec<PropertyOption> = states.iter().map(entity_option).collect();
    options.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then_with(|| a.value.cmp(&b.value)));
    options
}

/// Publishes the complete catalog and domain-specific pickers with identical labels and icons.
pub fn option_sources(states: &[State]) -> Vec<(&'static str, Vec<PropertyOption>)> {
    let all = entity_options(states);
    let subset = |domains: &[&str]| all.iter().filter(|o| domains.iter().any(|d| o.value.starts_with(d))).cloned().collect();
    vec![
        ("persons", subset(&["person.", "device_tracker."])),
        ("zones", subset(&["zone."])),
        ("calendars", subset(&["calendar."])),
        (ENTITIES_SOURCE, all),
    ]
}

#[cfg(test)]
pub fn test_state(entity: &str, state: &str, attributes: serde_json::Value) -> State {
    serde_json::from_value(serde_json::json!({
        "entity_id": entity, "state": state, "attributes": attributes,
        "last_changed": "2026-10-28T14:45:09.185007+00:00",
        "last_reported": "2026-10-28T14:45:09.185007+00:00",
        "last_updated": "2026-10-28T14:45:09.185007+00:00",
        "context": {"id": "01X", "parent_id": null, "user_id": null}
    })).expect("a valid state")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn options_use_friendly_name_id_and_icons() {
        let door = entity_option(&test_state("binary_sensor.front_door", "on", json!({"friendly_name": "Front door", "device_class": "door"})));
        assert_eq!(door, PropertyOption { value: "binary_sensor.front_door".into(), label: "Front door".into(), detail: "binary_sensor.front_door".into(), icon: Some("mdi:door-open".into()) });
        // No friendly name: the id is the label; an explicit icon attribute wins
        let bare = entity_option(&test_state("light.hall", "off", json!({"icon": "mdi:ceiling-light"})));
        assert_eq!((bare.label.as_str(), bare.icon.as_deref()), ("light.hall", Some("mdi:ceiling-light")));
        let temp = entity_option(&test_state("sensor.t", "21.5", json!({"friendly_name": "T", "device_class": "temperature", "unit_of_measurement": "°C"})));
        assert_eq!(temp.icon.as_deref(), Some("mdi:thermometer"));
        let odd = entity_option(&test_state("mystery.x", "1", json!({})));
        assert_eq!(odd.icon.as_deref(), Some("mdi:bookmark"));
    }

    #[test]
    fn options_are_sorted_by_label_ignoring_case() {
        let states = [
            test_state("light.b", "on", json!({"friendly_name": "bedroom"})),
            test_state("light.a", "on", json!({"friendly_name": "Attic"})),
            test_state("switch.c", "on", json!({"friendly_name": "Attic"})),
            test_state("sensor.z", "1", json!({})),
        ];
        let order: Vec<String> = entity_options(&states).into_iter().map(|o| o.value).collect();
        assert_eq!(order, ["light.a", "switch.c", "light.b", "sensor.z"]);
    }

    #[test]
    fn specialized_sources_keep_options_and_order_from_the_catalog() {
        let states = [
            test_state("person.me","home",json!({"friendly_name":"Alice"})),
            test_state("device_tracker.phone","home",json!({"friendly_name":"Phone"})),
            test_state("zone.home","1",json!({"friendly_name":"Home"})),
            test_state("calendar.work","on",json!({"friendly_name":"Work"})),
            test_state("light.hall","on",json!({})),
        ];
        let sets: std::collections::HashMap<_,_> = option_sources(&states).into_iter().collect();
        for (source,ids) in [("persons",vec!["person.me","device_tracker.phone"]),("zones",vec!["zone.home"]),("calendars",vec!["calendar.work"])] {
            assert_eq!(sets[source].iter().map(|o|o.value.as_str()).collect::<Vec<_>>(),ids);
            for option in &sets[source] { assert!(sets[ENTITIES_SOURCE].contains(option)); }
        }
        assert_eq!(sets[ENTITIES_SOURCE].len(),5);
        assert!(option_sources(&[]).iter().all(|(_,options)|options.is_empty()));
    }

}
