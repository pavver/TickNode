use ticknode_shared::*;
use super::wit;
use super::convert::decode_value;

/// A value emitted on a custom struct output pin must carry the tag of that exact type: a plugin
/// cannot emit another type or an untyped object disguised as `Named`.
fn check_output_type(definition: &NodeDefinition, pin: PinId, value: &DataValue) -> Result<(), String> {
    let declared = definition.pins.iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Output)
        .and_then(|p| match &p.pin_type {
            PinType::Data(DataType::Named(name)) => Some(name),
            _ => None,
        });
    match (declared, value) {
        (Some(name), DataValue::Object { type_name, .. }) if type_name.as_deref() != Some(name) => Err(format!(
            "The plugin emitted a value {} on an output of type {name}: the value was dropped",
            type_name.as_deref().map_or("without a type".to_string(), |t| format!("of type {t}")),
        )),
        _ => Ok(()),
    }
}

/// Checks the outputs a plugin pushed for a node outside of `execute`: each pin must be an output pin of the
/// matching kind (exec or data) and values must pass `check_output_type`. Returns the valid outputs and an
/// English error for every rejected one.
pub(super) fn validate_emission_outputs(definition: &NodeDefinition, outputs: Vec<wit::Output>) -> (Vec<NodeOutput>, Vec<String>) {
    let output_pin = |pin: u32| definition.pins.iter().find(|p| p.id == PinId(pin) && p.direction == PinDirection::Output);
    let (mut valid, mut errors) = (Vec::new(), Vec::new());
    for output in outputs {
        match output {
            wit::Output::Exec(pin) => match output_pin(pin) {
                Some(p) if p.pin_type == PinType::Exec => valid.push(NodeOutput::Execution(PinId(pin))),
                _ => errors.push(format!("The plugin emitted an exec pulse on pin {pin}, which is not an exec output of {}", definition.type_name)),
            },
            wit::Output::Clear(pin) => match output_pin(pin) {
                Some(p) if matches!(p.pin_type, PinType::Data(_)) => valid.push(NodeOutput::Clear(PinId(pin))),
                _ => errors.push(format!("The plugin cleared pin {pin}, which is not a data output of {}", definition.type_name)),
            },
            wit::Output::Data((pin, bytes)) => match output_pin(pin) {
                Some(p) if p.pin_type != PinType::Exec => {
                    let Some(value) = decode_value(&bytes) else { continue };
                    match check_output_type(definition, PinId(pin), &value) {
                        Ok(()) => valid.push(NodeOutput::Data(PinId(pin), value)),
                        Err(e) => errors.push(e),
                    }
                }
                _ => errors.push(format!("The plugin emitted a value on pin {pin}, which is not a data output of {}", definition.type_name)),
            },
        }
    }
    (valid, errors)
}
