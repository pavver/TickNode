//! Target-unit inference is performed only when selecting or explicitly refreshing an action.

use super::Service;
use ha_core_client::State;
use ticknode_plugin_sdk::DataType;

pub(super) fn infer_units(
    action: &str,
    service: &mut Service,
    state: Option<&State>,
    temperature_unit: Option<&str>,
) {
    let Some(state) = state else { return };
    let field_name = match action {
        "number.set_value" | "input_number.set_value" => "value",
        "climate.set_temperature" => "temperature",
        _ => return,
    };
    let Some((domain, _)) = action.split_once('.') else {
        return;
    };
    if crate::icons::domain_of(&state.entity_id) != domain {
        return;
    }
    let mapped = if domain == "climate" {
        let unit = state
            .attributes
            .get_as::<String>("temperature_unit")
            .or_else(|| state.attributes.get_as::<String>("unit_of_measurement"))
            .or_else(|| temperature_unit.map(str::to_owned));
        unit.and_then(|unit| crate::units::mapping(&unit, Some("temperature")))
            .filter(|(name, _)| *name == "quantity.temperature")
    } else {
        crate::units::state_unit(state)
    };
    let Some((name, unit)) = mapped else { return };
    for field in &mut service.fields {
        if field.name == field_name
            && field.data_type == DataType::Number
            && service.entity_unit_fields.contains(&field.name)
        {
            field.data_type = DataType::Named(name.into());
            field.unit = Some(unit.symbol.clone());
        }
    }
}
