//! The icon Home Assistant's frontend would show for an entity, as a Material Design Icons name
//! (`mdi:lightbulb`). An explicit `icon` attribute wins; otherwise the icon comes from the domain, the
//! `device_class` and the state, mirroring the frontend's `domain_icon` / `state_icon` helpers.
//! Only the common domains are covered; everything else falls back to `mdi:bookmark`.

pub const FALLBACK: &str = "mdi:bookmark";

/// The domain of an entity id (`light.kitchen` -> `light`).
pub fn domain_of(entity_id: &str) -> &str {
    entity_id.split_once('.').map_or(entity_id, |(domain, _)| domain)
}

/// Icon of an entity. `explicit` is its `icon` attribute (used as is when non-empty).
pub fn entity_icon(entity_id: &str, state: &str, device_class: Option<&str>, explicit: Option<&str>) -> String {
    if let Some(icon) = explicit.map(str::trim).filter(|i| !i.is_empty()) {
        return icon.to_string();
    }
    let domain = domain_of(entity_id);
    let class = device_class.unwrap_or_default();
    let on = state == "on";
    let name = match domain {
        "binary_sensor" => binary_sensor(class, on),
        "sensor" => sensor(class),
        "light" => if state == "off" { "lightbulb-off" } else { "lightbulb" },
        "switch" => match (class, state == "off") {
            ("outlet", true) => "power-plug-off",
            ("outlet", false) => "power-plug",
            (_, true) => "toggle-switch-variant-off",
            (_, false) => "toggle-switch-variant",
        },
        "input_boolean" => if state == "off" { "toggle-switch-off-outline" } else { "toggle-switch-outline" },
        "cover" => cover(class, state),
        "lock" => match state {
            "unlocked" | "open" | "opening" | "unlocking" => "lock-open-variant",
            "jammed" => "lock-alert",
            "locking" => "lock-clock",
            _ => "lock",
        },
        "person" | "device_tracker" => if state == "not_home" { "account-arrow-right" } else { "account" },
        "sun" => if state == "below_horizon" { "weather-night" } else { "white-balance-sunny" },
        "weather" => weather(state),
        "climate" => match state {
            "auto" => "thermostat-auto",
            "cool" => "snowflake",
            "dry" => "water-percent",
            "fan_only" => "fan",
            "heat" => "fire",
            "heat_cool" => "sun-snowflake-variant",
            "off" => "power",
            _ => "thermostat",
        },
        "media_player" => media_player(class, state),
        "automation" => if state == "off" { "robot-off" } else { "robot" },
        "script" => "script-text",
        "scene" => "palette",
        "fan" => if state == "off" { "fan-off" } else { "fan" },
        "humidifier" => if state == "off" { "air-humidifier-off" } else { "air-humidifier" },
        "update" => if on { "package-up" } else { "package" },
        "alarm_control_panel" => match state {
            "disarmed" => "shield-off",
            "triggered" => "bell-ring",
            "pending" | "arming" | "disarming" => "shield-outline",
            "armed_home" => "shield-home",
            "armed_night" => "shield-moon",
            "armed_away" | "armed_vacation" | "armed_custom_bypass" => "shield-lock",
            _ => "shield",
        },
        "vacuum" => if state == "docked" { "robot-vacuum" } else { "robot-vacuum-variant" },
        other => return domain_icon(other).to_string(),
    };
    format!("mdi:{name}")
}

/// Domain default (frontend fallback icons), `mdi:bookmark` for an unknown domain.
pub fn domain_icon(domain: &str) -> &'static str {
    match domain {
        "light" => "mdi:lightbulb",
        "switch" => "mdi:toggle-switch-variant",
        "fan" => "mdi:fan",
        "cover" => "mdi:window-shutter",
        "lock" => "mdi:lock",
        "media_player" => "mdi:cast",
        "automation" => "mdi:robot",
        "sensor" => "mdi:eye",
        "binary_sensor" => "mdi:radiobox-blank",
        "person" | "device_tracker" => "mdi:account",
        "sun" => "mdi:white-balance-sunny",
        "vacuum" => "mdi:robot-vacuum",
        "input_boolean" => "mdi:toggle-switch-outline",
        "humidifier" => "mdi:air-humidifier",
        "update" => "mdi:package",
        "air_quality" => "mdi:air-filter",
        "alarm_control_panel" => "mdi:shield",
        "alert" => "mdi:alert",
        "button" => "mdi:gesture-tap-button",
        "calendar" => "mdi:calendar",
        "camera" => "mdi:video",
        "climate" => "mdi:thermostat",
        "conversation" => "mdi:microphone-message",
        "counter" => "mdi:counter",
        "date" => "mdi:calendar",
        "datetime" => "mdi:calendar-clock",
        "event" => "mdi:calendar-star",
        "group" => "mdi:google-circles-communities",
        "homeassistant" => "mdi:home-assistant",
        "image" => "mdi:image",
        "input_button" => "mdi:button-pointer",
        "input_datetime" => "mdi:calendar-clock",
        "input_number" | "number" => "mdi:ray-vertex",
        "input_select" | "select" => "mdi:format-list-bulleted",
        "input_text" | "text" => "mdi:form-textbox",
        "lawn_mower" => "mdi:robot-mower",
        "notify" => "mdi:comment-alert",
        "persistent_notification" => "mdi:bell",
        "plant" => "mdi:flower",
        "proximity" => "mdi:apple-safari",
        "remote" => "mdi:remote",
        "schedule" => "mdi:calendar-clock",
        "siren" => "mdi:bullhorn",
        "stt" => "mdi:microphone-message",
        "tag" => "mdi:tag-outline",
        "time" => "mdi:clock",
        "timer" => "mdi:timer-outline",
        "todo" => "mdi:clipboard-list",
        "tts" => "mdi:speaker-message",
        "valve" => "mdi:valve",
        "water_heater" => "mdi:thermometer",
        "weather" => "mdi:weather-partly-cloudy",
        "zone" => "mdi:map-marker-radius",
        _ => FALLBACK,
    }
}

fn binary_sensor(class: &str, on: bool) -> &'static str {
    let pick = |on_icon, off_icon| if on { on_icon } else { off_icon };
    match class {
        "battery" => pick("battery-outline", "battery"),
        "battery_charging" => pick("battery-charging", "battery"),
        "carbon_monoxide" => pick("smoke-detector-alert", "smoke-detector"),
        "cold" => pick("snowflake", "thermometer"),
        "connectivity" => pick("check-network-outline", "close-network-outline"),
        "door" => pick("door-open", "door-closed"),
        "garage_door" => pick("garage-open", "garage"),
        "gas" | "problem" | "safety" | "tamper" => pick("alert-circle", "check-circle"),
        "heat" => pick("fire", "thermometer"),
        "light" => pick("brightness-7", "brightness-5"),
        "lock" => pick("lock-open", "lock"),
        "moisture" => pick("water", "water-off"),
        "motion" => pick("motion-sensor", "motion-sensor-off"),
        "occupancy" | "presence" => pick("home", "home-outline"),
        "opening" => pick("square-outline", "square"),
        "plug" | "power" => pick("power-plug", "power-plug-off"),
        "running" => pick("play", "stop"),
        "smoke" => pick("smoke-detector-variant-alert", "smoke-detector-variant"),
        "sound" => pick("music-note", "music-note-off"),
        "update" => pick("package-up", "package"),
        "vibration" => pick("vibrate", "crop-portrait"),
        "window" => pick("window-open", "window-closed"),
        _ => pick("checkbox-marked-circle", "radiobox-blank"),
    }
}

fn sensor(class: &str) -> &'static str {
    match class {
        "apparent_power" | "power" | "reactive_power" => "flash",
        "aqi" => "air-filter",
        "atmospheric_pressure" | "pressure" => "gauge",
        "battery" => "battery",
        "carbon_dioxide" => "molecule-co2",
        "carbon_monoxide" => "molecule-co",
        "current" => "current-ac",
        "data_size" => "database",
        "date" => "calendar",
        "distance" => "ruler",
        "duration" => "timer-outline",
        "energy" => "lightning-bolt",
        "frequency" | "voltage" => "sine-wave",
        "gas" => "meter-gas",
        "humidity" | "moisture" => "water-percent",
        "illuminance" => "brightness-5",
        "irradiance" => "sun-wireless",
        "monetary" => "cash",
        "nitrogen_dioxide" | "nitrogen_monoxide" | "nitrous_oxide" | "ozone" | "pm1" | "pm10" | "pm25" | "sulphur_dioxide" | "volatile_organic_compounds" => "molecule",
        "power_factor" => "angle-acute",
        "precipitation" | "precipitation_intensity" => "weather-rainy",
        "signal_strength" => "wifi",
        "sound_pressure" => "ear-hearing",
        "speed" => "speedometer",
        "temperature" => "thermometer",
        "timestamp" => "clock",
        "water" => "water",
        "weight" => "weight",
        "wind_speed" => "weather-windy",
        _ => "eye",
    }
}

fn cover(class: &str, state: &str) -> &'static str {
    let open = !matches!(state, "closed" | "closing");
    let pick = |open_icon, closed_icon| if open { open_icon } else { closed_icon };
    match class {
        "awning" => "awning-outline",
        "blind" => pick("blinds-open", "blinds"),
        "curtain" => pick("curtains-open", "curtains"),
        "damper" => pick("cog-transfer", "cog-off"),
        "door" => pick("door-open", "door-closed"),
        "garage" => pick("garage-open", "garage"),
        "gate" => pick("gate-open", "gate"),
        "window" => pick("window-open", "window-closed"),
        _ => pick("window-shutter-open", "window-shutter"),
    }
}

fn weather(state: &str) -> &'static str {
    match state {
        "clear-night" => "weather-night",
        "cloudy" => "weather-cloudy",
        "exceptional" => "alert-circle-outline",
        "fog" => "weather-fog",
        "hail" => "weather-hail",
        "lightning" => "weather-lightning",
        "lightning-rainy" => "weather-lightning-rainy",
        "partlycloudy" => "weather-partly-cloudy",
        "pouring" => "weather-pouring",
        "rainy" => "weather-rainy",
        "snowy" => "weather-snowy",
        "snowy-rainy" => "weather-snowy-rainy",
        "sunny" => "weather-sunny",
        "windy" => "weather-windy",
        "windy-variant" => "weather-windy-variant",
        _ => "weather-partly-cloudy",
    }
}

fn media_player(class: &str, state: &str) -> &'static str {
    let off = matches!(state, "off" | "unavailable");
    match (class, off) {
        ("tv", true) => "television-off",
        ("tv", false) => "television",
        ("speaker", true) => "speaker-off",
        ("speaker", false) => "speaker",
        ("receiver", true) => "audio-video-off",
        ("receiver", false) => "audio-video",
        (_, true) => "cast-off",
        (_, false) => if matches!(state, "playing" | "paused" | "buffering") { "cast-connected" } else { "cast" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon(entity: &str, state: &str, class: Option<&str>) -> String {
        entity_icon(entity, state, class, None)
    }

    #[test]
    fn explicit_icon_attribute_wins() {
        assert_eq!(entity_icon("light.a", "on", None, Some("mdi:ceiling-light")), "mdi:ceiling-light");
        // An empty attribute is not an icon
        assert_eq!(entity_icon("light.a", "on", None, Some("  ")), "mdi:lightbulb");
    }

    #[test]
    fn binary_sensors_follow_device_class_and_state() {
        assert_eq!(icon("binary_sensor.door", "on", Some("door")), "mdi:door-open");
        assert_eq!(icon("binary_sensor.door", "off", Some("door")), "mdi:door-closed");
        assert_eq!(icon("binary_sensor.m", "on", Some("motion")), "mdi:motion-sensor");
        assert_eq!(icon("binary_sensor.w", "off", Some("window")), "mdi:window-closed");
        assert_eq!(icon("binary_sensor.x", "on", None), "mdi:checkbox-marked-circle");
        assert_eq!(icon("binary_sensor.x", "off", Some("unknown_class")), "mdi:radiobox-blank");
    }

    #[test]
    fn sensors_use_device_class() {
        assert_eq!(icon("sensor.t", "21", Some("temperature")), "mdi:thermometer");
        assert_eq!(icon("sensor.h", "50", Some("humidity")), "mdi:water-percent");
        assert_eq!(icon("sensor.b", "90", Some("battery")), "mdi:battery");
        assert_eq!(icon("sensor.p", "3", Some("power")), "mdi:flash");
        assert_eq!(icon("sensor.other", "1", None), "mdi:eye");
    }

    #[test]
    fn domains_and_states() {
        assert_eq!(icon("light.a", "on", None), "mdi:lightbulb");
        assert_eq!(icon("light.a", "off", None), "mdi:lightbulb-off");
        assert_eq!(icon("switch.a", "on", None), "mdi:toggle-switch-variant");
        assert_eq!(icon("switch.a", "off", Some("outlet")), "mdi:power-plug-off");
        assert_eq!(icon("person.me", "home", None), "mdi:account");
        assert_eq!(icon("person.me", "not_home", None), "mdi:account-arrow-right");
        assert_eq!(icon("sun.sun", "below_horizon", None), "mdi:weather-night");
        assert_eq!(icon("weather.home", "rainy", None), "mdi:weather-rainy");
        assert_eq!(icon("climate.a", "heat", None), "mdi:fire");
        assert_eq!(icon("cover.a", "closed", None), "mdi:window-shutter");
        assert_eq!(icon("cover.a", "open", Some("garage")), "mdi:garage-open");
        assert_eq!(icon("lock.front", "locked", None), "mdi:lock");
        assert_eq!(icon("lock.front", "unlocked", None), "mdi:lock-open-variant");
        assert_eq!(icon("media_player.tv", "off", Some("tv")), "mdi:television-off");
        assert_eq!(icon("media_player.m", "playing", None), "mdi:cast-connected");
        assert_eq!(icon("automation.a", "off", None), "mdi:robot-off");
        assert_eq!(icon("script.a", "off", None), "mdi:script-text");
        assert_eq!(icon("input_boolean.a", "on", None), "mdi:toggle-switch-outline");
        assert_eq!(icon("input_number.a", "3", None), "mdi:ray-vertex");
        assert_eq!(icon("input_select.a", "x", None), "mdi:format-list-bulleted");
    }

    #[test]
    fn unknown_domains_fall_back_to_a_bookmark() {
        assert_eq!(icon("mystery.thing", "x", None), FALLBACK);
        assert_eq!(icon("no_dot", "x", None), FALLBACK);
    }
}
