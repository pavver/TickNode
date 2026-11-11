//! Home Assistant unit spellings and device classes mapped to the shared quantity catalog.

use ha_core_client::State;
use ticknode_plugin_sdk::quantity::{self, UnitDefinition};

fn class_quantity(class: &str) -> &'static [&'static str] {
    match class {
        "absolute_humidity" => &["quantity.concentration"],
        "apparent_power" => &["quantity.apparent_power"],
        "aqi" => &["quantity.aqi"],
        "area" => &["quantity.area"],
        "pressure" | "atmospheric_pressure" => &["quantity.pressure"],
        "battery" | "humidity" | "moisture" | "power_factor" => &["quantity.percentage"],
        "blood_glucose_concentration" => &["quantity.blood_glucose"],
        "carbon_dioxide" | "volatile_organic_compounds_parts" => &["quantity.parts"],
        "carbon_monoxide" => &["quantity.parts", "quantity.concentration"],
        "conductivity" => &["quantity.conductivity"],
        "current" => &["quantity.current"],
        "data_rate" => &["quantity.data_rate"],
        "data_size" => &["quantity.data_size"],
        "distance" | "precipitation" => &["quantity.length"],
        "energy" | "energy_storage" => &["quantity.energy"],
        "energy_distance" => &["quantity.energy_consumption", "quantity.energy_efficiency"],
        "frequency" => &["quantity.frequency"],
        "gas" | "volume" | "volume_storage" | "water" => &["quantity.volume"],
        "illuminance" => &["quantity.illuminance"],
        "irradiance" => &["quantity.irradiance"],
        "nitrogen_dioxide"
        | "nitrogen_monoxide"
        | "nitrous_oxide"
        | "ozone"
        | "sulphur_dioxide"
        | "pm1"
        | "pm25"
        | "pm10"
        | "volatile_organic_compounds" => &["quantity.concentration", "quantity.parts"],
        "power" => &["quantity.power"],
        "ph" => &["quantity.ph"],
        "precipitation_intensity" => &["quantity.precipitation_intensity"],
        "reactive_power" => &["quantity.reactive_power"],
        "reactive_energy" => &["quantity.reactive_energy"],
        "signal_strength" => &["quantity.signal_strength"],
        "sound_pressure" => &["quantity.sound_pressure"],
        "speed" => &["quantity.speed"],
        "wind_speed" => &["quantity.speed", "quantity.beaufort"],
        "temperature" => &["quantity.temperature"],
        "voltage" => &["quantity.voltage"],
        "volume_flow_rate" => &["quantity.volume_flow_rate"],
        "weight" => &["quantity.mass"],
        "wind_direction" => &["quantity.angle"],
        _ => &[],
    }
}

/// Seconds per HA duration unit; these units do not belong to the quantity catalog.
pub fn duration_factor(symbol: &str) -> Option<f64> {
    Some(match symbol.trim() {
        "d" => 86400.0,
        "h" => 3600.0,
        "min" => 60.0,
        "s" => 1.0,
        "ms" => 0.001,
        "µs" | "μs" | "us" => 0.000001,
        _ => return None,
    })
}

pub fn mapping(
    symbol: &str,
    device_class: Option<&str>,
) -> Option<(&'static str, &'static UnitDefinition)> {
    // These HA spellings are integration aliases; the catalog already includes µ and μ.
    let symbol = match symbol.trim() {
        "μg/m3" | "µg/m3" => "µg/m³",
        "m^3" => "m³",
        "ft^3" => "ft³",
        other => other,
    };
    let class = device_class.unwrap_or_default();
    if class == "monetary" {
        let currency = quantity::catalog()
            .iter()
            .filter(|definition| definition.name.starts_with("quantity.currency_"))
            .find_map(|definition| {
                let kind = definition.quantity()?;
                quantity::unit_in(kind, symbol, true).map(|unit| (definition.name.as_str(), unit))
            });
        if currency.is_some() {
            return currency;
        }
    }
    for &name in class_quantity(class) {
        let Some(kind) = quantity::kind(name) else {
            continue;
        };
        if symbol.is_empty() && matches!(class, "aqi" | "ph") {
            return kind.units.first().map(|unit| (name, unit));
        }
        if let Some(unit) =
            quantity::unit_in(kind, symbol, false).or_else(|| quantity::unit_in(kind, symbol, true))
        {
            return Some((name, unit));
        }
    }
    quantity::unit_by_symbol(symbol)
}

pub fn state_unit(state: &State) -> Option<(&'static str, &'static UnitDefinition)> {
    let symbol = state
        .attributes
        .get_as::<String>("unit_of_measurement")
        .unwrap_or_default();
    let class = state.attributes.get_as::<String>("device_class");
    mapping(&symbol, class.as_deref())
}

#[cfg(test)]
mod tests;
