//! Changes to Match inputs keep their dependent conditions consistent.

use super::*;

pub(super) fn validate_name(state: &MatchState, index: usize, new: &str) -> Result<String, String> {
    let new = new.trim().to_string();
    if !is_input_name(&new) {
        return Err(t!("match-error-bad-input-name", name = new));
    }
    if state
        .inputs
        .iter()
        .enumerate()
        .any(|(j, i)| j != index && i.name == new)
    {
        return Err(t!("match-error-duplicate-input", name = new));
    }
    Ok(new)
}

pub(super) fn rename_input(state: &mut MatchState, index: usize, new: String) {
    let old = std::mem::replace(&mut state.inputs[index].name, new.clone());
    for case in &mut state.cases {
        case.rename_input(&old, &new);
    }
}

pub(super) fn set_input_type(state: &mut MatchState, env: &Env, index: usize, ty: DataType) {
    let name = state.inputs[index].name.clone();
    state.inputs[index].value_type = ty.clone();
    // Auto follows the effective type of the connected value.
    let left = if ty == DataType::Any {
        type_of(&env.types, &Operand::Input(name.clone()))
    } else {
        ty
    };
    for condition in state
        .cases
        .iter_mut()
        .flat_map(|case| case.conditions.iter_mut())
    {
        if condition.left == Operand::Input(name.clone()) {
            fit_condition(&env.reg, condition, &left);
        }
    }
}

pub(super) fn remove_input(state: &mut MatchState, env: &Env, index: usize) {
    let gone = Operand::Input(state.inputs.remove(index).name);
    for case in &mut state.cases {
        case.conditions.retain(|condition| condition.left != gone);
        for condition in &mut case.conditions {
            if condition.right.as_ref() == Some(&gone) {
                condition.right = None;
                let ty = type_of(&env.types, &condition.left);
                fit_condition(&env.reg, condition, &ty);
            }
        }
    }
}

#[cfg(test)]
mod tests;
