use serde::{Deserialize, Serialize};
pub use uuid::Uuid;

/// Fingerprint of the plugin ABI: the WIT contract without comments/whitespace and the
/// checked-in `DataValue` serde layout snapshot. Plugins must be rebuilt when it changes.
pub const PLUGIN_API: &str = env!("TICKNODE_PLUGIN_API");

pub mod condition;
pub mod text;
pub mod plugin;
pub mod property;
pub mod types;
pub mod ui_message;
pub mod sheets;
pub mod settings;

pub use plugin::*;
pub use property::*;
pub use types::*;
pub use ui_message::*;
pub use sheets::*;
pub use settings::*;

pub mod variables;
pub use variables::*;

mod values;
mod definitions;
mod graph;
mod signatures;
pub use signatures::*;
mod protocol;
mod manifest;

pub use values::*;
pub use definitions::*;
pub use graph::*;
pub use protocol::*;
pub use manifest::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MathOp { Add, Sub, Mul, Div }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum LogOp { And, Or, Not }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeOutput {
    Execution(PinId),
    Data(PinId, DataValue),
    Variable(VariableCommand),
    /// Remove the current value from a data output.
    Clear(PinId),
}
