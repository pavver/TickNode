//! Canonical state labels by entity domain and device class.

pub(super) fn domain_canonical_states(
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
