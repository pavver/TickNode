//! Projection of Home Assistant trigger variables onto output pins.

use ha_core_client::TriggerFired;
use serde_json::Value;
use ticknode_plugin_sdk::{DataValue, Output, PinId};
use crate::convert::{json_to_value, state_value};
use super::{INFO, Platform, TRIGGERED, TriggerConfig};

fn object(value: &Value) -> DataValue {
    let json = match value {
        Value::Object(_) => value.clone(),
        Value::Null => serde_json::json!({}),
        _ => serde_json::json!({"value": value}),
    };
    json_to_value(&json).expect("an object always converts")
}

pub fn outputs(config: &TriggerConfig, fired: &TriggerFired) -> Vec<Output> {
    let raw = &fired.trigger;
    let mut outputs = vec![Output::Data(INFO, object(raw))];
    let string = |value: &Value| DataValue::from(value.as_str().unwrap_or_default());
    let values = match config.platform {
        Platform::State | Platform::NumericState | Platform::Zone => {
            let entity = fired.entity_id().unwrap_or_default();
            let to = fired.to_state();
            let mut values = vec![
                Some(DataValue::from(entity)),
                fired.from_state().as_ref().map(state_value),
                to.as_ref().map(state_value),
            ];
            if config.platform == Platform::NumericState {
                let number = to
                    .as_ref()
                    .and_then(|state| {
                        if config.attribute.is_empty() {
                            state.state.parse::<f64>().ok()
                        } else {
                            state.attributes.get(&config.attribute).and_then(|v| v.as_f64().or_else(|| v.as_str()?.parse::<f64>().ok()))
                        }
                    })
                    .filter(|n| n.is_finite());
                values.push(number.map(DataValue::from));
            }
            outputs.extend(values.into_iter().enumerate().map(|(i, v)| {
                let pin = PinId(i as u32 + 2);
                v.map_or(Output::Clear(pin), |value| Output::Data(pin, value))
            }));
            outputs.push(Output::Exec(TRIGGERED));
            return outputs;
        }
        Platform::Event => vec![object(&raw["event"]["data"]), string(&raw["event"]["event_type"])],
        Platform::Mqtt => vec![string(&raw["topic"]), string(&raw["payload"])],
        Platform::Webhook => vec![object(raw.get("json").filter(|v| !v.is_null()).unwrap_or(&raw["data"])), object(&raw["query"])],
        Platform::Calendar => vec![object(&raw["calendar_event"])],
        Platform::Tag => vec![string(&raw["event"]["data"]["tag_id"]), string(&raw["event"]["data"]["device_id"])],
        Platform::Conversation => vec![string(&raw["sentence"]), object(&raw["slots"])],
        _ => vec![DataValue::from(fired.description().unwrap_or_default())],
    };
    outputs.extend(values.into_iter().enumerate().map(|(i, v)| Output::Data(PinId(i as u32 + 2), v)));
    outputs.push(Output::Exec(TRIGGERED));
    outputs
}
