//! Service discovery and the property-backed action form. Snapshots preserve positional pins offline.

mod catalog;
mod node;
mod options;

pub use catalog::*;
pub use node::ActionNode;
pub use options::*;

use crate::convert::{json_to_value, value_to_json};
use serde_json::{Value, json};
use ticknode_plugin_sdk::*;

// Reserve the high IDs so service fields can grow from input pin 2 without moving outputs.
pub const DONE: PinId = PinId(u32::MAX - 3);
pub const FAILED: PinId = PinId(u32::MAX - 2);
pub const ERROR: PinId = PinId(u32::MAX - 1);
pub const RESPONSE: PinId = PinId(u32::MAX);

pub fn field_json(value: &DataValue) -> Value {
    match value {
        DataValue::Color(c) => json!([c.r, c.g, c.b]),
        DataValue::Duration(d) => {
            json!({"hours": d.as_secs() / 3600, "minutes": d.as_secs() / 60 % 60, "seconds": d.as_secs_f64() % 60.0})
        }
        other => value_to_json(other),
    }
}

impl Field {
    /// Quantity inputs carry canonical numbers; HA expects the snapshotted selector's unit.
    pub fn json(&self, value: &DataValue) -> Option<Value> {
        if let (DataType::Named(name), DataValue::Number(n), Some(symbol)) =
            (&self.data_type, value, &self.unit)
            && let Some(kind) = quantity::kind(name)
            && let Some(unit) = quantity::unit_in(kind, symbol, false)
                .or_else(|| quantity::unit_in(kind, symbol, true))
        {
            let converted = quantity::from_canonical(n.to_f64(), unit);
            return (n.to_f64().is_finite() && converted.is_finite()).then(|| json!(converted));
        }
        Some(field_json(value))
    }
}

pub fn outputs(result: Result<Value, String>) -> Vec<Output> {
    match result {
        Ok(result) => {
            let mut outputs = vec![Output::Data(ERROR, "".into()), Output::Exec(DONE)];
            if let Some(response) = result
                .get("response")
                .and_then(json_to_value)
                .filter(|v| matches!(v, DataValue::Object { .. }))
            {
                outputs.push(Output::Data(RESPONSE, response));
            }
            outputs
        }
        Err(e) => vec![Output::Data(ERROR, e.into()), Output::Exec(FAILED)],
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod quantity_tests;
