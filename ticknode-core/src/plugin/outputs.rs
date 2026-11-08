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
    let (mut valid, mut errors) = (Vec::new(), Vec::new());
    for output in outputs {
        match validate_output(definition, output) {
            Ok(Some(output)) => valid.push(output),
            Ok(None) => {},
            Err(error) => errors.push(error),
        }
    }
    (valid, errors)
}

fn validate_output(definition: &NodeDefinition, output: wit::Output) -> Result<Option<NodeOutput>, String> {
    match output {
        wit::Output::Exec(pin) => {
            require_output_pin(definition, pin, true, "emitted an exec pulse on", "an exec")?;
            Ok(Some(NodeOutput::Execution(PinId(pin))))
        }
        wit::Output::Clear(pin) => {
            require_output_pin(definition, pin, false, "cleared", "a data")?;
            Ok(Some(NodeOutput::Clear(PinId(pin))))
        }
        wit::Output::Data((pin, bytes)) => {
            require_output_pin(definition, pin, false, "emitted a value on", "a data")?;
            decode_output(definition, pin, &bytes)
        }
    }
}

fn require_output_pin(definition: &NodeDefinition, pin: u32, exec: bool, action: &str, kind: &str) -> Result<(), String> {
    let output = definition.pins.iter().find(|p| p.id == PinId(pin) && p.direction == PinDirection::Output);
    if output.is_some_and(|p| (p.pin_type == PinType::Exec) == exec) {
        Ok(())
    } else {
        Err(format!("The plugin {action} pin {pin}, which is not {kind} output of {}", definition.type_name))
    }
}

fn decode_output(definition: &NodeDefinition, pin: u32, bytes: &[u8]) -> Result<Option<NodeOutput>, String> {
    let Some(value) = decode_value(bytes) else { return Ok(None) };
    check_output_type(definition, PinId(pin), &value)?;
    Ok(Some(NodeOutput::Data(PinId(pin), value)))
}
