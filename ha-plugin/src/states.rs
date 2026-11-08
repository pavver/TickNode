//! State and attribute autocomplete options for state triggers and condition gates.

use ha_core_client::State;
use serde_json::Value;
use ticknode_plugin_sdk::PropertyOption;

use crate::icons::domain_of;

pub const STATES_SOURCE: &str = "states";
pub const ATTRIBUTES_SOURCE: &str = "attributes";

pub fn state_source(entity_id: &str) -> String {
    let entity_id = entity_id.trim();
    if entity_id.is_empty() {
        STATES_SOURCE.into()
    } else {
        format!("{STATES_SOURCE}:{entity_id}")
    }
}

pub fn attribute_source(entity_id: &str) -> String {
    let entity_id = entity_id.trim();
    if entity_id.is_empty() {
        ATTRIBUTES_SOURCE.into()
    } else {
        format!("{ATTRIBUTES_SOURCE}:{entity_id}")
    }
}

fn domain_canonical_states(
    domain: &str,
    device_class: Option<&str>,
) -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    match domain {
        "light" | "switch" | "fan" | "humidifier" | "siren" | "automation" | "script"
        | "input_boolean" => vec![
            ("on", "Увімкнено (on)", Some("mdi:power")),
            ("off", "Вимкнено (off)", Some("mdi:power-off")),
        ],
        "cover" => vec![
            ("open", "Відкрито (open)", Some("mdi:arrow-up")),
            ("closed", "Закрито (closed)", Some("mdi:arrow-down")),
            ("opening", "Відкривається (opening)", Some("mdi:arrow-up-bold")),
            ("closing", "Закривається (closing)", Some("mdi:arrow-down-bold")),
            ("stopped", "Зупинено (stopped)", Some("mdi:stop")),
        ],
        "lock" => vec![
            ("locked", "Заблоковано (locked)", Some("mdi:lock")),
            ("unlocked", "Розблоковано (unlocked)", Some("mdi:lock-open")),
            ("locking", "Блокується (locking)", Some("mdi:lock-clock")),
            ("unlocking", "Розблоковується (unlocking)", Some("mdi:lock-clock")),
            ("jammed", "Заїло (jammed)", Some("mdi:lock-alert")),
        ],
        "climate" => vec![
            ("heat", "Обігрів (heat)", Some("mdi:fire")),
            ("cool", "Охолодження (cool)", Some("mdi:snowflake")),
            (
                "heat_cool",
                "Авто обігрів/охолодження (heat_cool)",
                Some("mdi:autorenew"),
            ),
            ("auto", "Авто (auto)", Some("mdi:calendar-sync")),
            ("dry", "Осушення (dry)", Some("mdi:water-percent")),
            ("fan_only", "Лише вентилятор (fan_only)", Some("mdi:fan")),
            ("off", "Вимкнено (off)", Some("mdi:power")),
        ],
        "media_player" => vec![
            ("playing", "Відтворюється (playing)", Some("mdi:play")),
            ("paused", "Пауза (paused)", Some("mdi:pause")),
            ("idle", "Очікування (idle)", Some("mdi:sleep")),
            (
                "standby",
                "Режим очікування (standby)",
                Some("mdi:power-standby"),
            ),
            (
                "buffering",
                "Буферизація (buffering)",
                Some("mdi:progress-clock"),
            ),
            ("on", "Увімкнено (on)", Some("mdi:power")),
            ("off", "Вимкнено (off)", Some("mdi:power-off")),
        ],
        "vacuum" => vec![
            ("docked", "На базі (docked)", Some("mdi:home-map-marker")),
            ("cleaning", "Прибирання (cleaning)", Some("mdi:robot-vacuum")),
            ("paused", "Пауза (paused)", Some("mdi:pause")),
            (
                "returning",
                "Повернення на базу (returning)",
                Some("mdi:home-import-outline"),
            ),
            ("error", "Помилка (error)", Some("mdi:alert-circle")),
            ("idle", "Очікування (idle)", Some("mdi:sleep")),
        ],
        "alarm_control_panel" => vec![
            (
                "disarmed",
                "Знято з охорони (disarmed)",
                Some("mdi:shield-off"),
            ),
            (
                "armed_home",
                "Вдома під охороною (armed_home)",
                Some("mdi:shield-home"),
            ),
            (
                "armed_away",
                "Повна охорона (armed_away)",
                Some("mdi:shield-lock"),
            ),
            (
                "armed_night",
                "Нічна охорона (armed_night)",
                Some("mdi:shield-moon"),
            ),
            (
                "armed_vacation",
                "Охорона у відпустці (armed_vacation)",
                Some("mdi:shield-airplane"),
            ),
            ("triggered", "Тривога (triggered)", Some("mdi:bell-ring")),
            (
                "arming",
                "Постановка на охорону (arming)",
                Some("mdi:shield-sync"),
            ),
            (
                "disarming",
                "Зняття з охорони (disarming)",
                Some("mdi:shield-sync"),
            ),
            ("pending", "Очікування (pending)", Some("mdi:shield-outline")),
        ],
        "person" | "device_tracker" => vec![
            ("home", "Вдома (home)", Some("mdi:home-account")),
            (
                "not_home",
                "Не вдома (not_home)",
                Some("mdi:account-arrow-right"),
            ),
        ],
        "valve" => vec![
            ("open", "Відкрито (open)", Some("mdi:pipe-valve")),
            ("closed", "Закрито (closed)", Some("mdi:pipe-valve")),
            ("opening", "Відкривається (opening)", Some("mdi:arrow-up")),
            ("closing", "Закривається (closing)", Some("mdi:arrow-down")),
        ],
        "timer" => vec![
            ("active", "Активний (active)", Some("mdi:timer-play")),
            ("paused", "На паузі (paused)", Some("mdi:timer-pause")),
            ("idle", "Очікування (idle)", Some("mdi:timer-stop")),
        ],
        "update" => vec![
            ("on", "Є оновлення (on)", Some("mdi:package-up")),
            ("off", "Оновлено (off)", Some("mdi:check-circle")),
        ],
        "binary_sensor" => match device_class {
            Some("door" | "window" | "garage_door" | "opening") => vec![
                ("on", "Відкрито (on)", Some("mdi:door-open")),
                ("off", "Закрито (off)", Some("mdi:door-closed")),
            ],
            Some("motion" | "occupancy" | "presence") => vec![
                (
                    "on",
                    "Виявлено рух / присутність (on)",
                    Some("mdi:motion-sensor"),
                ),
                ("off", "Немає руху (off)", Some("mdi:motion-sensor-off")),
            ],
            Some("moisture") => vec![
                ("on", "Витік води! (on)", Some("mdi:water-alert")),
                ("off", "Сухо (off)", Some("mdi:water-check")),
            ],
            Some("smoke" | "gas") => vec![
                ("on", "Виявлено дим / газ! (on)", Some("mdi:fire-alert")),
                ("off", "Чисто (off)", Some("mdi:check")),
            ],
            Some("lock") => vec![
                ("on", "Розблоковано (on)", Some("mdi:lock-open")),
                ("off", "Заблоковано (off)", Some("mdi:lock")),
            ],
            Some("plug" | "power") => vec![
                (
                    "on",
                    "Підключено до живлення (on)",
                    Some("mdi:power-plug"),
                ),
                (
                    "off",
                    "Відключено від живлення (off)",
                    Some("mdi:power-plug-off"),
                ),
            ],
            Some("battery_charging") => vec![
                (
                    "on",
                    "Заряджається (on)",
                    Some("mdi:battery-charging"),
                ),
                ("off", "Не заряджається (off)", Some("mdi:battery")),
            ],
            Some("connectivity") => vec![
                ("on", "Підключено (on)", Some("mdi:check-network")),
                ("off", "Відключено (off)", Some("mdi:close-network")),
            ],
            Some("problem" | "tamper") => vec![
                ("on", "Проблема / саботаж! (on)", Some("mdi:alert-circle")),
                ("off", "У нормі (off)", Some("mdi:check-circle")),
            ],
            _ => vec![
                (
                    "on",
                    "Увімкнено / Спрацював (on)",
                    Some("mdi:checkbox-marked-circle"),
                ),
                (
                    "off",
                    "Вимкнено / Спокійний (off)",
                    Some("mdi:close-circle"),
                ),
            ],
        },
        _ => vec![
            ("on", "Увімкнено (on)", Some("mdi:power")),
            ("off", "Вимкнено (off)", Some("mdi:power-off")),
            ("open", "Відкрито (open)", Some("mdi:arrow-up")),
            ("closed", "Закрито (closed)", Some("mdi:arrow-down")),
            ("home", "Вдома (home)", Some("mdi:home-account")),
            (
                "not_home",
                "Не вдома (not_home)",
                Some("mdi:account-arrow-right"),
            ),
        ],
    }
}

pub fn state_options(
    entity_id: &str,
    state: Option<&State>,
    allow_empty: bool,
) -> Vec<PropertyOption> {
    let mut options = Vec::new();
    if allow_empty {
        options.push(PropertyOption {
            value: String::new(),
            label: "Будь-який стан (не вказано)".into(),
            detail: "Спрацьовуватиме на будь-яку зміну".into(),
            icon: Some("mdi:circle-outline".into()),
        });
    }

    let domain = domain_of(entity_id.trim());
    let device_class = state.and_then(|s| s.attributes.get_as::<String>("device_class"));

    // If current state is known, add it prominently
    if let Some(s) = state
        && !s.state.is_empty()
        && s.state != "unavailable"
        && s.state != "unknown"
    {
        options.push(PropertyOption {
            value: s.state.clone(),
            label: format!("Поточний стан ({})", s.state),
            detail: entity_id.to_string(),
            icon: Some("mdi:history".into()),
        });
    }

    // Add canonical domain states (skipping value if it matches the current state already added)
    for (value, label, icon) in domain_canonical_states(domain, device_class.as_deref()) {
        if !options.iter().any(|o| o.value == value) {
            options.push(PropertyOption {
                value: value.into(),
                label: label.into(),
                detail: format!("{domain}.{value}"),
                icon: icon.map(str::to_string),
            });
        }
    }

    // Universal special states
    for (value, label) in [
        ("unavailable", "Недоступний (unavailable)"),
        ("unknown", "Невідомий (unknown)"),
    ] {
        if !options.iter().any(|o| o.value == value) {
            options.push(PropertyOption {
                value: value.into(),
                label: label.into(),
                detail: value.into(),
                icon: Some("mdi:help-circle-outline".into()),
            });
        }
    }

    options
}

pub fn attribute_options(_entity_id: &str, state: Option<&State>) -> Vec<PropertyOption> {
    let mut options = vec![PropertyOption {
        value: String::new(),
        label: "Основний стан (без атрибута)".into(),
        detail: "Перевіряти стан сутності".into(),
        icon: Some("mdi:circle-outline".into()),
    }];

    if let Some(state) = state {
        let json = serde_json::to_value(&state.attributes).unwrap_or(Value::Null);
        let inner = match json {
            Value::Object(map) if map.len() == 1 => map.into_iter().next().map_or(Value::Null, |(_, v)| v),
            other => other,
        };
        if let Some(attrs) = inner.as_object() {
            let mut keys: Vec<_> = attrs.keys().collect();
            keys.sort();
            for key in keys {
                let val_str = match &attrs[key] {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    other => other.to_string(),
                };
                options.push(PropertyOption {
                    value: key.clone(),
                    label: key.clone(),
                    detail: format!("Значення: {val_str}"),
                    icon: Some("mdi:code-tags".into()),
                });
            }
        }
    }

    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn state_options_for_light_include_on_off_and_current() {
        let state = crate::entities::test_state("light.hall", "on", json!({}));
        let options = state_options("light.hall", Some(&state), true);
        assert_eq!(options[0].value, "");
        assert_eq!(options[1].value, "on");
        assert!(options[1].label.contains("Поточний стан"));
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
        assert!(options.iter().any(|o| o.value == "on" && o.label.contains("Відкрито")));
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
}
