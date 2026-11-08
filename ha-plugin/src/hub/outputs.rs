//! Output pins and result mapping for state changes and low-level requests.

use ha_core_client::{State, StateChange};
use ticknode_plugin_sdk::{Output, PinId};
use crate::convert::state_value;

/// Output pins of `ha/state_changed`.
pub const CHANGED: PinId = PinId(0);
pub const NEW: PinId = PinId(1);
pub const OLD: PinId = PinId(2);
/// Output pins of `ha/get_state`.
pub const GET_DONE: PinId = PinId(2);
pub const GET_FAILED: PinId = PinId(3);
pub const GET_STATE: PinId = PinId(4);
pub const GET_ERROR: PinId = PinId(5);
/// Output pins of `ha/call_service`.
pub const CALL_DONE: PinId = PinId(5);
pub const CALL_FAILED: PinId = PinId(6);
pub const CALL_ERROR: PinId = PinId(7);

/// Outputs of a `ha/state_changed` pulse. Missing states clear their output pins.
pub fn change_outputs(change: &StateChange) -> Vec<Output> {
    let value = |pin, state: &Option<State>| state.as_ref().map_or(Output::Clear(pin), |s| Output::Data(pin, state_value(s)));
    vec![value(NEW, &change.new_state), value(OLD, &change.old_state), Output::Exec(CHANGED)]
}

pub fn get_state_outputs(result: Result<State, String>) -> Vec<Output> {
    match result {
        Ok(state) => vec![Output::Data(GET_STATE, state_value(&state)), Output::Data(GET_ERROR, "".into()), Output::Exec(GET_DONE)],
        Err(error) => vec![Output::Clear(GET_STATE), Output::Data(GET_ERROR, error.into()), Output::Exec(GET_FAILED)],
    }
}

pub fn service_outputs(result: Result<(), String>) -> Vec<Output> {
    match result {
        Ok(()) => vec![Output::Data(CALL_ERROR, "".into()), Output::Exec(CALL_DONE)],
        Err(error) => vec![Output::Data(CALL_ERROR, error.into()), Output::Exec(CALL_FAILED)],
    }
}
