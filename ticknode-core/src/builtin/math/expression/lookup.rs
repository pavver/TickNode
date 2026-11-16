use super::*;

pub(super) fn lookup_pin_value(
    pins: &[(String, PinId, DataType)],
    inputs: &HashMap<PinId, Option<DataValue>>,
    var_name: &str,
) -> Option<FormulaValue> {
    let (_, pin, _) = pins.iter().find(|(n, _, _)| n == var_name)?;
    match inputs.get(pin) {
        Some(Some(DataValue::Number(n))) => Some(FormulaValue::Number(*n)),
        Some(Some(DataValue::Bool(b))) => Some(FormulaValue::Bool(*b)),
        _ => Some(FormulaValue::Number(NumberValue::Int(0))),
    }
}

pub(super) fn lookup_var_value(
    ctx: &NodeContext,
    expr_len: usize,
    vref: &VariableRef,
) -> Result<FormulaValue, FormulaError> {
    let value = if vref.lookup == VariableLookup::Plugin {
        ctx.plugin_values.resolve(&vref.name).map(|(v, _)| v)
    } else {
        ctx.variable(&vref.name, vref.lookup).map(|v| &v.value)
    };
    let value = value.ok_or_else(|| {
        FormulaError::new(
            0..expr_len,
            "error-variable-missing",
            format!("Variable \"{}\" not found", vref.name),
        )
        .with_arg("name", &vref.name)
    })?;
    match value {
        DataValue::Number(n) => Ok(FormulaValue::Number(*n)),
        DataValue::Bool(b) => Ok(FormulaValue::Bool(*b)),
        _ => Err(FormulaError::new(
            0..expr_len,
            "error-formula-variable-type",
            format!("Variable '{}' has non-numeric type", vref.name),
        )
        .with_arg("name", &vref.name)),
    }
}

pub(super) fn restore_pin_ids(elements: Vec<DataValue>) -> Vec<(String, PinId, DataType)> {
    let mut pins = Vec::new();
    for val in elements {
        let DataValue::String(s) = val else { continue };
        if let Some((name, pin, ty)) = parse_pin_id_element(&s)
            && !pins
                .iter()
                .any(|(n, p, _): &(String, PinId, DataType)| *n == name || *p == pin)
        {
            pins.push((name, pin, ty));
        }
    }
    pins
}

fn parse_pin_id_element(s: &str) -> Option<(String, PinId, DataType)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() < 2 {
        return None;
    }
    let pin = parts[0]
        .parse::<u32>()
        .ok()
        .filter(|&p| (1..=0x7fff_ffff).contains(&p))?;
    let var_name = parts[1].to_string();
    let ty = if parts.len() >= 3 {
        parse_data_type(parts[2]).unwrap_or(DataType::Number)
    } else {
        DataType::Number
    };
    Some((var_name, PinId(pin), ty))
}
