use super::convert::decode_value;
use super::wit;
use ticknode_shared::*;

/// A value emitted on a custom struct output pin must carry the tag of that exact type: a plugin
/// cannot emit another type or an untyped object disguised as `Named`.
fn check_output_type(
    types: &TypeRegistry,
    definition: &NodeDefinition,
    pin: PinId,
    value: &DataValue,
) -> Result<(), String> {
    if let Some(PinDefinition {
        pin_type: PinType::Data(t),
        ..
    }) = definition
        .pins
        .iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Output)
        && !types.identifier_value_fits(t, value)
    {
        return Err(format!(
            "The plugin emitted a non-integer value on an identifier output of {}: the value was dropped",
            definition.type_name
        ));
    }
    let declared = definition
        .pins
        .iter()
        .find(|p| p.id == pin && p.direction == PinDirection::Output)
        .and_then(|p| match &p.pin_type {
            PinType::Data(DataType::Named(name)) => Some(name),
            _ => None,
        });
    if let Some(name) = declared
        && quantity::kind(name).is_some()
        && !matches!(value, DataValue::Number(n) if n.to_f64().is_finite())
    {
        return Err(format!(
            "The plugin emitted a non-numeric value on an output of type {name}: the value was dropped"
        ));
    }
    match (declared, value) {
        (Some(name), DataValue::Object { type_name, .. }) if type_name.as_deref() != Some(name) => {
            Err(format!(
                "The plugin emitted a value {} on an output of type {name}: the value was dropped",
                type_name
                    .as_deref()
                    .map_or("without a type".to_string(), |t| format!("of type {t}")),
            ))
        }
        _ => Ok(()),
    }
}

/// Checks the outputs a plugin pushed for a node outside of `execute`: each pin must be an output pin of the
/// matching kind (exec or data) and values must pass `check_output_type`. Returns the valid outputs and an
/// English error for every rejected one.
pub(super) fn validate_emission_outputs(
    types: &TypeRegistry,
    definition: &NodeDefinition,
    outputs: Vec<wit::Output>,
) -> (Vec<NodeOutput>, Vec<String>) {
    let (mut valid, mut errors) = (Vec::new(), Vec::new());
    for output in outputs {
        match validate_output(types, definition, output) {
            Ok(Some(output)) => valid.push(output),
            Ok(None) => {}
            Err(error) => errors.push(error),
        }
    }
    (valid, errors)
}

fn validate_output(
    types: &TypeRegistry,
    definition: &NodeDefinition,
    output: wit::Output,
) -> Result<Option<NodeOutput>, String> {
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
            decode_output(types, definition, pin, &bytes)
        }
    }
}

fn require_output_pin(
    definition: &NodeDefinition,
    pin: u32,
    exec: bool,
    action: &str,
    kind: &str,
) -> Result<(), String> {
    let output = definition
        .pins
        .iter()
        .find(|p| p.id == PinId(pin) && p.direction == PinDirection::Output);
    if output.is_some_and(|p| (p.pin_type == PinType::Exec) == exec) {
        Ok(())
    } else {
        Err(format!(
            "The plugin {action} pin {pin}, which is not {kind} output of {}",
            definition.type_name
        ))
    }
}

fn decode_output(
    types: &TypeRegistry,
    definition: &NodeDefinition,
    pin: u32,
    bytes: &[u8],
) -> Result<Option<NodeOutput>, String> {
    let Some(value) = decode_value(bytes) else {
        return Ok(None);
    };
    check_output_type(types, definition, PinId(pin), &value)?;
    Ok(Some(NodeOutput::Data(PinId(pin), value)))
}

pub(super) fn ui_error(error: &str) -> UiMessage {
    if error.starts_with("The plugin emitted a non-integer value on an identifier output") {
        UiMessage::new(
            "error-plugin-identifier-output",
            "The plugin returned an invalid identifier value",
        )
    } else {
        UiMessage::plain(error)
    }
}

#[cfg(test)]
mod quantity_tests {
    use super::*;
    #[test]
    fn quantity_outputs_accept_canonical_numbers_and_reject_other_representations() {
        let definition = NodeDefinition {
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "Power".into(),
                direction: PinDirection::Output,
                pin_type: PinType::Data(DataType::Named("quantity.power".into())),
                required: false,
            }],
            ..Default::default()
        };
        assert!(
            check_output_type(&TypeRegistry::new(), &definition, PinId(0), &2000i64.into()).is_ok()
        );
        assert!(
            check_output_type(&TypeRegistry::new(), &definition, PinId(0), &1.5.into()).is_ok()
        );
        assert!(
            check_output_type(&TypeRegistry::new(), &definition, PinId(0), &"2 kW".into()).is_err()
        );
        assert!(
            check_output_type(
                &TypeRegistry::new(),
                &definition,
                PinId(0),
                &DataValue::Bool(true)
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod identifier_tests {
    use super::*;
    #[test]
    fn identifier_outputs_reject_floats_and_other_values_including_nested_lists() {
        let mut types = TypeRegistry::new();
        types
            .insert_all(&[TypeDefinition::identifier("test.id", "ID", "")])
            .unwrap();
        let t = DataType::Named("test.id".into());
        let mut definition = NodeDefinition {
            pins: vec![PinDefinition {
                id: PinId(0),
                name: "ID".into(),
                direction: PinDirection::Output,
                pin_type: PinType::Data(t.clone()),
                required: false,
            }],
            ..Default::default()
        };
        for value in [i64::MIN.into(), i64::MAX.into()] {
            assert!(check_output_type(&types, &definition, PinId(0), &value).is_ok());
        }
        for value in [
            DataValue::Number(NumberValue::Float(1.0)),
            f64::NAN.into(),
            "123".into(),
            true.into(),
            DataValue::Object {
                fields: Default::default(),
                type_name: Some("test.id".into()),
            },
        ] {
            let error = check_output_type(&types, &definition, PinId(0), &value).unwrap_err();
            assert_eq!(ui_error(&error).key, "error-plugin-identifier-output");
        }
        definition.pins[0].pin_type = PinType::Data(DataType::List(Box::new(t.clone())));
        let value = DataValue::List {
            element_type: t,
            elements: vec![1i64.into(), DataValue::Number(NumberValue::Float(2.0))],
        };
        assert!(check_output_type(&types, &definition, PinId(0), &value).is_err());
    }
}
