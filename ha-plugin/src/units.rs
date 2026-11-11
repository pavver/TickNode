//! Home Assistant unit spellings and device classes mapped to the shared quantity catalog.

use ha_core_client::State;
use ticknode_plugin_sdk::quantity::{self, UnitDefinition};

fn class_quantity(class: &str) -> Option<&'static str> {
    Some(match class {
        "power" => "quantity.power",
        "energy" => "quantity.energy",
        "voltage" => "quantity.voltage",
        "current" => "quantity.current",
        "apparent_power" => "quantity.apparent_power",
        "frequency" => "quantity.frequency",
        "temperature" => "quantity.temperature",
        "battery" | "humidity" | "moisture" => "quantity.percentage",
        "pressure" | "atmospheric_pressure" => "quantity.pressure",
        "speed" | "wind_speed" => "quantity.speed",
        "distance" => "quantity.length",
        "volume" | "water" | "gas" => "quantity.volume",
        "weight" => "quantity.mass",
        "illuminance" => "quantity.illuminance",
        "irradiance" => "quantity.irradiance",
        "pm1" | "pm25" | "pm10" | "volatile_organic_compounds" => "quantity.concentration",
        "carbon_dioxide" | "carbon_monoxide" | "volatile_organic_compounds_parts" => {
            "quantity.parts"
        }
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
    if let Some(name) = device_class.and_then(class_quantity) {
        let kind = quantity::kind(name)?;
        let unit = quantity::unit_in(kind, symbol, false)
            .or_else(|| quantity::unit_in(kind, symbol, true))?;
        return Some((name, unit));
    }
    quantity::unit_by_symbol(symbol)
}

pub fn state_unit(state: &State) -> Option<(&'static str, &'static UnitDefinition)> {
    let symbol = state.attributes.get_as::<String>("unit_of_measurement")?;
    let class = state.attributes.get_as::<String>("device_class");
    mapping(&symbol, class.as_deref())
}

#[cfg(test)]
mod tests;
