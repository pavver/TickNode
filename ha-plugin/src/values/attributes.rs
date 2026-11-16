//! Recursive attribute conversion with HA-local units and color conventions.
use super::{Contracts, object};
use serde_json::Value;
use ticknode_shared::{Color, DataType, DataValue, quantity};

pub(super) fn convert(
    json: &Value,
    temperature: Option<&str>,
    contracts: &mut Contracts,
    path: &str,
    depth: usize,
) -> Option<DataValue> {
    if depth > 16 {
        return None;
    }
    let key = path.rsplit('.').next().unwrap_or(path);
    if let Some(text) = json.as_str()
        && let Ok(time) = chrono::DateTime::parse_from_rfc3339(text)
    {
        contracts.insert(path.into(), DataType::DateTime);
        return Some(DataValue::DateTime(time.to_utc()));
    }
    if matches!(key, "rgb_color" | "hs_color")
        && let Some(color) = color(key, json)
    {
        contracts.insert(path.into(), DataType::Color);
        return Some(DataValue::Color(color));
    }
    if let Value::Object(fields) = json {
        let temperature = fields
            .get("temperature_unit")
            .and_then(Value::as_str)
            .or(temperature);
        return Some(object(fields.iter().filter_map(|(key, val)| {
            let path = format!("{path}.{key}");
            if let Some(n) = val.as_f64() {
                if matches!(
                    key.as_str(),
                    "temperature"
                        | "current_temperature"
                        | "apparent_temperature"
                        | "dew_point"
                        | "target_temp_low"
                        | "target_temp_high"
                        | "target_temp_step"
                        | "min_temp"
                        | "max_temp"
                ) {
                    contracts.insert(path.clone(), DataType::Named("quantity.temperature".into()));
                    let (name, unit) = crate::units::mapping(temperature?, Some("temperature"))?;
                    let canonical = quantity::to_canonical(n, unit);
                    contracts.insert(path.clone(), DataType::Named(name.into()));
                    return canonical
                        .is_finite()
                        .then(|| (key.clone(), canonical.into()));
                }
                if matches!(
                    key.as_str(),
                    "color_temp_kelvin"
                        | "min_color_temp_kelvin"
                        | "max_color_temp_kelvin"
                        | "color_temp"
                        | "min_mireds"
                        | "max_mireds"
                ) {
                    let kind = quantity::kind("quantity.color_temperature")?;
                    let unit = quantity::unit_in(
                        kind,
                        if key.contains("kelvin") { "K" } else { "mired" },
                        true,
                    )?;
                    contracts.insert(
                        path.clone(),
                        DataType::Named("quantity.color_temperature".into()),
                    );
                    let canonical = quantity::to_canonical(n, unit);
                    return canonical
                        .is_finite()
                        .then(|| (key.clone(), canonical.into()));
                }
                if matches!(
                    key.as_str(),
                    "latitude" | "longitude" | "azimuth" | "elevation" | "course" | "wind_bearing"
                ) {
                    contracts.insert(path.clone(), DataType::Named("quantity.angle".into()));
                    return Some((key.clone(), n.into()));
                }
                if let Some((class, unit)) = unit_for(key, fields, temperature)
                    && let Some((name, unit)) = crate::units::mapping(unit, Some(class))
                {
                    let n = quantity::to_canonical(n, unit);
                    if n.is_finite() {
                        contracts.insert(path.clone(), DataType::Named(name.into()));
                        return Some((key.clone(), n.into()));
                    }
                }
                if matches!(key.as_str(), "media_duration" | "media_position")
                    && let Some(delta) = ticknode_shared::delta_from_secs_f64(n)
                {
                    contracts.insert(path.clone(), DataType::Duration);
                    return Some((key.clone(), DataValue::Duration(delta)));
                }
            }
            Some((
                key.clone(),
                convert(val, temperature, contracts, &path, depth + 1)?,
            ))
        })));
    }
    if let Value::Array(elements) = json {
        let mut converted = Vec::new();
        let mut element_types = Vec::new();
        // Element contracts belong to the list type, never numeric catalog paths.
        for value in elements {
            let mut element_contracts = Contracts::new();
            if let Some(mut value) =
                convert(value, temperature, &mut element_contracts, path, depth + 1)
            {
                super::stamp(&mut value, path, &element_contracts);
                element_types.push(
                    element_contracts
                        .get(path)
                        .cloned()
                        .unwrap_or_else(|| value.get_type()),
                );
                converted.push(value);
            }
        }
        let elements = converted;
        let element_type = element_types
            .first()
            .cloned()
            .filter(|ty| element_types.iter().all(|t| t == ty))
            .unwrap_or(DataType::Any);
        return Some(DataValue::List {
            element_type,
            elements,
        });
    }
    crate::convert::json_to_value(json)
}
fn unit_for<'a>(
    key: &str,
    fields: &'a serde_json::Map<String, Value>,
    temperature: Option<&'a str>,
) -> Option<(&'static str, &'a str)> {
    let unit = |key: &str| fields.get(key).and_then(Value::as_str);
    Some(match key {
        "temperature"
        | "current_temperature"
        | "apparent_temperature"
        | "dew_point"
        | "target_temp_low"
        | "target_temp_high"
        | "target_temp_step"
        | "min_temp"
        | "max_temp" => ("temperature", temperature?),
        "color_temp_kelvin" | "min_color_temp_kelvin" | "max_color_temp_kelvin" => {
            ("temperature", "K")
        }
        "color_temp" | "min_mireds" | "max_mireds" => ("temperature", "mired"),
        "humidity"
        | "current_humidity"
        | "target_humidity"
        | "min_humidity"
        | "max_humidity"
        | "battery_level"
        | "cloud_coverage"
        | "percentage"
        | "percentage_step"
        | "current_position"
        | "current_tilt_position"
        | "update_percentage"
        | "precipitation_probability" => ("humidity", "%"),
        "pressure" => ("pressure", unit("pressure_unit")?),
        "wind_speed" | "wind_gust_speed" => ("wind_speed", unit("wind_speed_unit")?),
        "visibility" => ("distance", unit("visibility_unit")?),
        "precipitation" => ("precipitation", unit("precipitation_unit")?),
        "radius" | "gps_accuracy" | "altitude" | "vertical_accuracy" => ("distance", "m"),
        _ => return None,
    })
}
fn color(key: &str, json: &Value) -> Option<Color> {
    let parts = json.as_array()?;
    let (r, g, b) = if key == "rgb_color" {
        if parts.len() != 3 {
            return None;
        }
        (
            parts[0].as_u64()?.min(255) as u8,
            parts[1].as_u64()?.min(255) as u8,
            parts[2].as_u64()?.min(255) as u8,
        )
    } else {
        if parts.len() != 2 {
            return None;
        }
        let h = parts[0].as_f64()?.rem_euclid(360.0) / 60.0;
        let s = (parts[1].as_f64()? / 100.0).clamp(0.0, 1.0);
        if !h.is_finite() || !s.is_finite() {
            return None;
        }
        let x = s * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
        let (r, g, b) = match h as u8 {
            0 => (s, x, 0.0),
            1 => (x, s, 0.0),
            2 => (0.0, s, x),
            3 => (0.0, x, s),
            4 => (x, 0.0, s),
            _ => (s, 0.0, x),
        };
        let byte = |v: f64| ((v + 1.0 - s) * 255.0).round() as u8;
        (byte(r), byte(g), byte(b))
    };
    Some(Color { r, g, b, a: 255 })
}
