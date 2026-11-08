//! Action picker sources, entity filtering and usage-frequency ordering.

use std::collections::BTreeMap;
use ticknode_plugin_sdk::PropertyOption;
use crate::icons::domain_icon;
use super::Service;

pub fn action_source(target: &str) -> String {
    let target = target.trim();
    if target.is_empty() {
        "actions".into()
    } else {
        format!("actions:{target}")
    }
}

/// Whether `service` can be executed for `target` (`Some((entity_id, state))` when an entity is selected,
/// or `None` for actions that run without an entity).
pub fn service_matches(
    service: &Service,
    target: Option<(&str, Option<&ha_core_client::State>)>,
) -> bool {
    match target {
        Some((entity_id, state)) => {
            let domain = crate::icons::domain_of(entity_id.trim());
            !domain.is_empty()
                && service.targeted
                && service.filters.iter().any(|f| f.matches(domain, state))
        }
        None => !service.targeted,
    }
}

/// Usage-frequency rank of a Home Assistant action (smaller = more frequently used).
pub fn action_rank(id: &str) -> u16 {
    let (domain, name) = id.split_once('.').unwrap_or(("", id));
    match name {
        "turn_on" | "open_cover" | "open_valve" | "lock" => 0,
        "turn_off" | "close_cover" | "close_valve" | "unlock" => 1,
        "stop_cover" | "stop_valve" => 2,
        "toggle" | "open" => 3,
        "close" | "stop" => 4,
        "press" => 5,
        "trigger" => 6,
        "activate" | "apply" => 7,
        "set_value" => 20,
        "select_option" => 21,
        "set_datetime" => 22,
        "set_temperature" => 23,
        "set_hvac_mode" => 24,
        "set_preset_mode" => 25,
        "set_percentage" => 26,
        "set_cover_position" | "set_valve_position" => 27,
        "set_humidity" => 28,
        "set_fan_mode" | "set_swing_mode" | "set_direction" => 29,
        "play_media" | "media_play_pause" => 40,
        "media_play" => 41,
        "media_pause" => 42,
        "media_stop" => 43,
        "volume_set" => 44,
        "volume_up" | "volume_down" | "volume_mute" => 45,
        "media_next_track" | "media_previous_track" => 46,
        "select_source" | "select_sound_mode" => 47,
        "start" | "start_mowing" => 60,
        "pause" => 61,
        "cancel" => 62,
        "finish" => 63,
        "return_to_base" | "dock" => 64,
        "increment" => 65,
        "decrement" => 66,
        "reset" => 67,
        "arm_away" | "alarm_arm_away" => 70,
        "arm_home" | "alarm_arm_home" => 71,
        "arm_night" | "alarm_arm_night" => 72,
        "disarm" | "alarm_disarm" => 73,
        "send_message" | "notify" | "speak" => 80,
        "create" | "dismiss" => 81,
        "add_item" | "update_item" | "remove_item" | "get_items" => 82,
        "get_forecasts" | "get_events" => 83,
        _ if name.starts_with("set_") || name.starts_with("select_") => 39,
        _ if name.starts_with("open_") || name.starts_with("close_") => 19,
        "update_entity" => 200,
        _ if name == "reload" || name.starts_with("reload_") => 201,
        _ if matches!(domain, "script" | "notify") => 90,
        _ => 100,
    }
}

pub fn options_for(
    services: &BTreeMap<String, Service>,
    target: Option<(&str, Option<&ha_core_client::State>)>,
) -> Vec<PropertyOption> {
    let entity_domain = target.map(|(entity_id, _)| crate::icons::domain_of(entity_id.trim()));
    let mut options: Vec<_> = services
        .iter()
        .filter(|(_, service)| service_matches(service, target))
        .map(|(id, service)| PropertyOption {
            value: id.clone(),
            label: service.label.clone(),
            detail: id.clone(),
            icon: Some(domain_icon(crate::icons::domain_of(id)).into()),
        })
        .collect();
    options.sort_by_key(|o| {
        let service_domain = crate::icons::domain_of(&o.value);
        let tier = match entity_domain {
            Some(d) if service_domain == d => 0u8,
            Some(_) => 1u8,
            None => 0u8,
        };
        (tier, action_rank(&o.value), o.label.to_lowercase(), o.value.clone())
    });
    options
}
