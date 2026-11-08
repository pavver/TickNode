//! Low-level get-state and call-service request node adapters.

use std::time::Instant;
use ticknode_plugin_sdk::*;
use crate::convert::{STATE_TYPE, value_to_json};
use crate::hub::{CALL_DONE, CALL_ERROR, CALL_FAILED, GET_DONE, GET_ERROR, GET_FAILED, GET_STATE};
use crate::{CATEGORY, with_hub};

// --- ha/get_state ---

pub(super) struct GetState {
    pub(super) node: NodeId,
}

impl PluginNode for GetState {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            properties: Vec::new(),
            type_name: "ha/get_state".into(),
            title: "HA Get State".into(),
            category: CATEGORY.into(),
            description: "Reads the current state of a Home Assistant entity".into(),
            help: "On a signal at In asks Home Assistant for the current state of Entity ID. The answer arrives a little later (a few ticks): \
on success State holds the entity and a signal goes to Done; on failure (not connected, no such entity, timeout) Error holds the reason and a signal goes to Failed. \
After a failure State is cleared.\n\nThe request fails at once if the connection to Home Assistant is down.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Entity ID", DataType::String).required(),
                exec_out(GET_DONE.0, "Done"),
                exec_out(GET_FAILED.0, "Failed"),
                data_out(GET_STATE.0, "State", DataType::Named(STATE_TYPE.into())),
                data_out(GET_ERROR.0, "Error", DataType::String),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let entity = inputs.string(PinId(1)).trim().to_string();
        with_hub(|hub, env| hub.get_state(self.node, &entity, Instant::now(), env)).unwrap_or_default()
    }
}

// --- ha/call_service ---

pub(super) struct CallService {
    pub(super) node: NodeId,
}

impl PluginNode for CallService {
    fn definition(&self) -> NodeDefinition {
        NodeDefinition {
            properties: Vec::new(),
            type_name: "ha/call_service".into(),
            title: "HA Call Service".into(),
            category: CATEGORY.into(),
            description: "Calls a Home Assistant service".into(),
            help: "Low-level variant of Action: use ha/action for a searchable action form with typed field pins. On a signal at In calls the Home Assistant service Domain.Service (for example light + turn_on). \
Entity ID is the optional target entity; Data is an optional object with the service data (for example {brightness: 120}).\n\n\
When Home Assistant confirms the call a signal goes to Done; otherwise (not connected, rejected, timeout) Error holds the reason and a signal goes to Failed. \
The answer arrives a little later (a few ticks); the request fails at once if the connection is down.".into(),
            pins: vec![
                exec_in(0, "In"),
                data_in(1, "Domain", DataType::String).required(),
                data_in(2, "Service", DataType::String).required(),
                data_in(3, "Entity ID", DataType::String),
                data_in(4, "Data", DataType::Object),
                exec_out(CALL_DONE.0, "Done"),
                exec_out(CALL_FAILED.0, "Failed"),
                data_out(CALL_ERROR.0, "Error", DataType::String),
            ],
        }
    }

    fn execute(&mut self, _: Trigger, inputs: &Inputs) -> Vec<Output> {
        let domain = inputs.string(PinId(1)).trim().to_string();
        let service = inputs.string(PinId(2)).trim().to_string();
        let entity = inputs.string(PinId(3)).trim().to_string();
        let data = inputs.get(PinId(4)).map_or(serde_json::Value::Null, value_to_json);
        with_hub(|hub, env| hub.call_service(self.node, &domain, &service, &entity, data, Instant::now(), env)).unwrap_or_default()
    }
}
