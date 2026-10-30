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
}
