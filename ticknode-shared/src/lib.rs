use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

/// Fingerprint of the plugin ABI: the WIT contract without comments/whitespace and the
/// checked-in `DataValue` serde layout snapshot. Plugins must be rebuilt when it changes.
pub const PLUGIN_API: &str = env!("TICKNODE_PLUGIN_API");

pub mod condition;
pub mod convert;
pub mod quantity;
pub mod text;
pub mod to_text;
pub use quantity::{DisplayRules, NumberDisplay, QuantityDisplay, Rounding, UnitStep};
pub mod plugin;
pub mod property;
pub mod settings;
pub mod sheets;
pub mod types;
pub mod ui_message;

pub use plugin::*;
pub use property::*;
pub use settings::*;
pub use sheets::*;
pub use types::*;
pub use ui_message::*;

pub mod variables;
pub use variables::*;

pub mod template;
pub use template::*;

mod definitions;
mod graph;
mod signatures;
mod values;
pub use signatures::*;
mod manifest;
mod protocol;

pub use definitions::*;
pub use graph::*;
pub use manifest::*;
pub use protocol::*;
pub use values::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LogOp {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeOutput {
    Execution(PinId),
    Data(PinId, DataValue),
    Variable(VariableCommand),
    /// Remove the current value from a data output.
    Clear(PinId),
}
