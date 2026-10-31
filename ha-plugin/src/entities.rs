//! The entity picker options: one per Home Assistant entity, built from a `get_states` answer.

use std::collections::BTreeMap;

use ha_core_client::State;
use ticknode_plugin_sdk::PropertyOption;

use crate::actions::{Service, service_matches};
use crate::icons::entity_icon;

/// Name of the option source used by the `entity_id` property of `ha/state_changed`.
pub const ENTITIES_SOURCE: &str = "entities";

/// Name of the option source used by the `target` property of `ha/action` (filtered to entities with actions).
pub const ACTION_TARGETS_SOURCE: &str = "action_targets";

/// Checks if an entity is technical/diagnostic noise.
fn is_diagnostic(
    domain: &str,
    name: &str,
    device_class: Option<&str>,
    entity_category: Option<&str>,
) -> bool {
    if matches!(entity_category, Some("diagnostic" | "config")) {
        return true;
    }
    if matches!(
        domain,
        "update" | "device_tracker" | "event" | "stt" | "tts" | "conversation"
    ) {
        return true;
    }
    if let Some(dc) = device_class {
        if matches!(
            dc,
            "battery"
                | "battery_charging"
                | "signal_strength"
                | "voltage"
                | "current"
                | "power_factor"
                | "timestamp"
                | "duration"
        ) {
            return true;
        }
    }
    let lower = name.to_lowercase();
    lower.ends_with("_battery")
        || lower.ends_with("_battery_level")
        || lower.ends_with("_linkquality")
        || lower.ends_with("_lqi")
        || lower.ends_with("_rssi")
        || lower.ends_with("_signal")
        || lower.ends_with("_uptime")
        || lower.ends_with("_ip_address")
        || lower.ends_with("_ip")
        || lower.ends_with("_mac")
        || lower.ends_with("_cpu")
        || lower.ends_with("_memory")
        || lower.ends_with("_ram")
        || lower.ends_with("_disk")
        || lower.ends_with("_ping")
        || lower.ends_with("_latency")
}

/// Checks if a sensor represents a real physical measurement.
fn is_physical_sensor(device_class: Option<&str>, name: &str) -> bool {
    if let Some(dc) = device_class {
        matches!(
            dc,
            "temperature"
                | "humidity"
                | "illuminance"
                | "pressure"
                | "power"
                | "energy"
                | "carbon_dioxide"
                | "carbon_monoxide"
                | "pm25"
                | "pm10"
                | "aqi"
                | "distance"
                | "speed"
                | "volume"
                | "water"
                | "weight"
                | "wind_speed"
                | "precipitation"
        )
    } else {
        let lower = name.to_lowercase();
        lower.contains("temp")
            || lower.contains("humid")
            || lower.contains("power")
            || lower.contains("energy")
            || lower.contains("light")
            || lower.contains("lux")
            || lower.contains("press")
            || lower.contains("co2")
    }
}

/// Priority rank of an entity (smaller = higher priority / more frequently used in automations).
pub fn entity_rank(state: &State) -> u8 {
    let id = &state.entity_id;
    let (domain, name) = id.split_once('.').unwrap_or(("", id));
    let device_class: Option<String> = state.attributes.get_as("device_class");
    let entity_category: Option<String> = state.attributes.get_as("entity_category");

    if is_diagnostic(
        domain,
        name,
        device_class.as_deref(),
        entity_category.as_deref(),
    ) {
        return 4;
    }

    match domain {
        // Rank 0: Primary controllable actuators & automations
        "light" | "switch" | "climate" | "cover" | "lock" | "media_player" | "fan"
        | "vacuum" | "valve" | "humidifier" | "siren" | "alarm_control_panel"
        | "scene" | "script" | "automation" | "camera" | "remote" => 0,

        // Rank 1: Primary input helpers & controls
        _ if domain.starts_with("input_") || matches!(domain, "timer" | "counter") => 1,

        // Rank 2: Meaningful environment, security & presence sensors
        "binary_sensor" | "person" | "weather" | "sun" | "calendar" | "zone" => 2,
        "sensor" if is_physical_sensor(device_class.as_deref(), name) => 2,

        // Rank 3: Secondary sensors, utility numbers/selects, notification, etc.
        _ => 3,
    }
}

/// Option of one entity: value and detail are the entity id, the label is its `friendly_name`
/// (the entity id when there is none), the icon is the `icon` attribute or the frontend's default.
pub fn entity_option(state: &State) -> PropertyOption {
    let id = &state.entity_id;
    let label = state.attributes.friendly_name().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(id);
    let device_class: Option<String> = state.attributes.get_as("device_class");
    let icon = entity_icon(id, &state.state, device_class.as_deref(), state.attributes.icon());
    PropertyOption { value: id.clone(), label: label.to_string(), detail: id.clone(), icon: Some(icon) }
}

/// Options of all entities, sorted by priority rank, then by label (case-insensitive), then by entity id.
pub fn entity_options(states: &[State]) -> Vec<PropertyOption> {
    let mut items: Vec<(u8, PropertyOption)> = states
        .iter()
        .map(|s| (entity_rank(s), entity_option(s)))
        .collect();
    items.sort_by(|(r1, opt1), (r2, opt2)| {
        r1.cmp(r2)
            .then_with(|| opt1.label.to_lowercase().cmp(&opt2.label.to_lowercase()))
            .then_with(|| opt1.value.cmp(&opt2.value))
    });
    items.into_iter().map(|(_, opt)| opt).collect()
}

/// Options of entities that can execute at least one action from the services catalog.
pub fn action_target_options(
    states: &[State],
    services: &BTreeMap<String, Service>,
) -> Vec<PropertyOption> {
    let filtered: Vec<&State> = if services.is_empty() {
        states.iter().collect()
    } else {
        states
            .iter()
            .filter(|state| {
                services
                    .values()
                    .any(|s| service_matches(s, Some((&state.entity_id, Some(state)))))
            })
            .collect()
    };
    let mut items: Vec<(u8, PropertyOption)> = filtered
        .into_iter()
        .map(|s| (entity_rank(s), entity_option(s)))
        .collect();
    items.sort_by(|(r1, opt1), (r2, opt2)| {
        r1.cmp(r2)
            .then_with(|| opt1.label.to_lowercase().cmp(&opt2.label.to_lowercase()))
            .then_with(|| opt1.value.cmp(&opt2.value))
    });
    items.into_iter().map(|(_, opt)| opt).collect()
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

    #[test]
    fn options_are_prioritized_by_rank_putting_actuators_first_and_noise_last() {
        let states = [
            test_state("update.firmware", "on", json!({"friendly_name": "Firmware update"})),
            test_state("sensor.plug_rssi", "-65", json!({"friendly_name": "Plug signal", "device_class": "signal_strength"})),
            test_state("sensor.temperature", "21.0", json!({"friendly_name": "Living room temp", "device_class": "temperature"})),
            test_state("binary_sensor.door", "off", json!({"friendly_name": "Front door", "device_class": "door"})),
            test_state("input_boolean.vacation", "off", json!({"friendly_name": "Vacation mode"})),
            test_state("light.kitchen", "on", json!({"friendly_name": "Kitchen light"})),
        ];
        let order: Vec<String> = entity_options(&states).into_iter().map(|o| o.value).collect();
        assert_eq!(
            order,
            [
                "light.kitchen",               // Rank 0 (actuator)
                "input_boolean.vacation",      // Rank 1 (helper)
                "binary_sensor.door",          // Rank 2 (door sensor, "Front door")
                "sensor.temperature",          // Rank 2 (temp sensor, "Living room temp")
                "update.firmware",             // Rank 4 (noise: update, "Firmware update")
                "sensor.plug_rssi",            // Rank 4 (noise: signal, "Plug signal")
            ]
        );
    }

    #[test]
    fn action_targets_filters_out_entities_without_actions() {
        let services = crate::actions::services(&json!({
            "light": {"turn_on": {"name": "Turn on", "target": {"entity": [{"domain": "light"}]}}},
            "switch": {"turn_on": {"name": "Turn on", "target": {"entity": [{"domain": "switch"}]}}}
        }));
        let states = [
            test_state("light.kitchen", "on", json!({"friendly_name": "Kitchen light"})),
            test_state("sensor.temperature", "21.0", json!({"friendly_name": "Temperature", "device_class": "temperature"})),
            test_state("switch.fan", "off", json!({"friendly_name": "Ceiling fan"})),
        ];
        let targets: Vec<String> = action_target_options(&states, &services)
            .into_iter()
            .map(|o| o.value)
            .collect();
        // sensor.temperature has no actions and is omitted; switch.fan and light.kitchen stay, sorted by name
        assert_eq!(targets, ["switch.fan", "light.kitchen"]);
    }
}
