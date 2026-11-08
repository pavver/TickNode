//! Canonical state labels by entity domain and device class.

pub(super) fn domain_canonical_states(
    domain: &str,
    device_class: Option<&str>,
) -> Vec<(&'static str, &'static str, Option<&'static str>)> {
    match domain {
        "light" | "switch" | "fan" | "humidifier" | "siren" | "automation" | "script"
        | "input_boolean" => vec![
            ("on", "On (on)", Some("mdi:power")),
            ("off", "Off (off)", Some("mdi:power-off")),
        ],
        "cover" => vec![
            ("open", "Open (open)", Some("mdi:arrow-up")),
            ("closed", "Closed (closed)", Some("mdi:arrow-down")),
            ("opening", "Opening (opening)", Some("mdi:arrow-up-bold")),
            ("closing", "Closing (closing)", Some("mdi:arrow-down-bold")),
            ("stopped", "Stopped (stopped)", Some("mdi:stop")),
        ],
        "lock" => vec![
            ("locked", "Locked (locked)", Some("mdi:lock")),
            ("unlocked", "Unlocked (unlocked)", Some("mdi:lock-open")),
            ("locking", "Locking (locking)", Some("mdi:lock-clock")),
            ("unlocking", "Unlocking (unlocking)", Some("mdi:lock-clock")),
            ("jammed", "Jammed (jammed)", Some("mdi:lock-alert")),
        ],
        "climate" => vec![
            ("heat", "Heat (heat)", Some("mdi:fire")),
            ("cool", "Cool (cool)", Some("mdi:snowflake")),
            (
                "heat_cool",
                "Heat/Cool (heat_cool)",
                Some("mdi:autorenew"),
            ),
            ("auto", "Auto (auto)", Some("mdi:calendar-sync")),
            ("dry", "Dry (dry)", Some("mdi:water-percent")),
            ("fan_only", "Fan only (fan_only)", Some("mdi:fan")),
            ("off", "Off (off)", Some("mdi:power")),
        ],
        "media_player" => vec![
            ("playing", "Playing (playing)", Some("mdi:play")),
            ("paused", "Paused (paused)", Some("mdi:pause")),
            ("idle", "Idle (idle)", Some("mdi:sleep")),
            (
                "standby",
                "Standby (standby)",
                Some("mdi:power-standby"),
            ),
            (
                "buffering",
                "Buffering (buffering)",
                Some("mdi:progress-clock"),
            ),
            ("on", "On (on)", Some("mdi:power")),
            ("off", "Off (off)", Some("mdi:power-off")),
        ],
        "vacuum" => vec![
            ("docked", "Docked (docked)", Some("mdi:home-map-marker")),
            ("cleaning", "Cleaning (cleaning)", Some("mdi:robot-vacuum")),
            ("paused", "Paused (paused)", Some("mdi:pause")),
            (
                "returning",
                "Returning to dock (returning)",
                Some("mdi:home-import-outline"),
            ),
            ("error", "Error (error)", Some("mdi:alert-circle")),
            ("idle", "Idle (idle)", Some("mdi:sleep")),
        ],
        "alarm_control_panel" => vec![
            (
                "disarmed",
                "Disarmed (disarmed)",
                Some("mdi:shield-off"),
            ),
            (
                "armed_home",
                "Armed home (armed_home)",
                Some("mdi:shield-home"),
            ),
            (
                "armed_away",
                "Armed away (armed_away)",
                Some("mdi:shield-lock"),
            ),
            (
                "armed_night",
                "Armed night (armed_night)",
                Some("mdi:shield-moon"),
            ),
            (
                "armed_vacation",
                "Armed vacation (armed_vacation)",
                Some("mdi:shield-airplane"),
            ),
            ("triggered", "Triggered (triggered)", Some("mdi:bell-ring")),
            (
                "arming",
                "Arming (arming)",
                Some("mdi:shield-sync"),
            ),
            (
                "disarming",
                "Disarming (disarming)",
                Some("mdi:shield-sync"),
            ),
            ("pending", "Pending (pending)", Some("mdi:shield-outline")),
        ],
        "person" | "device_tracker" => vec![
            ("home", "Home (home)", Some("mdi:home-account")),
            (
                "not_home",
                "Away (not_home)",
                Some("mdi:account-arrow-right"),
            ),
        ],
        "valve" => vec![
            ("open", "Open (open)", Some("mdi:pipe-valve")),
            ("closed", "Closed (closed)", Some("mdi:pipe-valve")),
            ("opening", "Opening (opening)", Some("mdi:arrow-up")),
            ("closing", "Closing (closing)", Some("mdi:arrow-down")),
        ],
        "timer" => vec![
            ("active", "Active (active)", Some("mdi:timer-play")),
            ("paused", "Paused (paused)", Some("mdi:timer-pause")),
            ("idle", "Idle (idle)", Some("mdi:timer-stop")),
        ],
        "update" => vec![
            ("on", "Update available (on)", Some("mdi:package-up")),
            ("off", "Up-to-date (off)", Some("mdi:check-circle")),
        ],
        "binary_sensor" => match device_class {
            Some("door" | "window" | "garage_door" | "opening") => vec![
                ("on", "Open (on)", Some("mdi:door-open")),
                ("off", "Closed (off)", Some("mdi:door-closed")),
            ],
            Some("motion" | "occupancy" | "presence") => vec![
                (
                    "on",
                    "Motion / presence detected (on)",
                    Some("mdi:motion-sensor"),
                ),
                ("off", "Clear (off)", Some("mdi:motion-sensor-off")),
            ],
            Some("moisture") => vec![
                ("on", "Water leak! (on)", Some("mdi:water-alert")),
                ("off", "Dry (off)", Some("mdi:water-check")),
            ],
            Some("smoke" | "gas") => vec![
                ("on", "Smoke / gas detected! (on)", Some("mdi:fire-alert")),
                ("off", "Clear (off)", Some("mdi:check")),
            ],
            Some("lock") => vec![
                ("on", "Unlocked (on)", Some("mdi:lock-open")),
                ("off", "Locked (off)", Some("mdi:lock")),
            ],
            Some("plug" | "power") => vec![
                (
                    "on",
                    "Connected to power (on)",
                    Some("mdi:power-plug"),
                ),
                (
                    "off",
                    "Disconnected from power (off)",
                    Some("mdi:power-plug-off"),
                ),
            ],
            Some("battery_charging") => vec![
                (
                    "on",
                    "Charging (on)",
                    Some("mdi:battery-charging"),
                ),
                ("off", "Not charging (off)", Some("mdi:battery")),
            ],
            Some("connectivity") => vec![
                ("on", "Connected (on)", Some("mdi:check-network")),
                ("off", "Disconnected (off)", Some("mdi:close-network")),
            ],
            Some("problem" | "tamper") => vec![
                ("on", "Problem / tamper! (on)", Some("mdi:alert-circle")),
                ("off", "OK (off)", Some("mdi:check-circle")),
            ],
            _ => vec![
                (
                    "on",
                    "On / Triggered (on)",
                    Some("mdi:checkbox-marked-circle"),
                ),
                (
                    "off",
                    "Off / Idle (off)",
                    Some("mdi:close-circle"),
                ),
            ],
        },
        _ => vec![
            ("on", "On (on)", Some("mdi:power")),
            ("off", "Off (off)", Some("mdi:power-off")),
            ("open", "Open (open)", Some("mdi:arrow-up")),
            ("closed", "Closed (closed)", Some("mdi:arrow-down")),
            ("home", "Home (home)", Some("mdi:home-account")),
            (
                "not_home",
                "Away (not_home)",
                Some("mdi:account-arrow-right"),
            ),
        ],
    }
}
