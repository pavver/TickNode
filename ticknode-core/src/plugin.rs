//! WASM plugins: wasmtime components implementing the `wit/plugin.wit` contract.
//!
//! One component instance (`Store`) per plugin; each graph node is a `node` resource within it,
//! so node state persists across calls. Calls are bounded by fuel and memory limits. If a plugin
//! traps (crash, out of fuel), the instance is recreated and nodes are reconstructed from their
//! properties on the next call. The node that caused the failure is halted until a property changes
//! so it does not trap on every tick.

use std::sync::{Arc, Mutex, MutexGuard};
use ticknode_shared::*;
use crate::RemovedConnections;

mod bindings;
mod host;
mod runtime;
mod options;
mod node;
mod outputs;
mod convert;
mod loading;
mod lifecycle;
mod catalog;
mod emissions;

use bindings::exports::ticknode::plugin::nodes::Guest as NodesApi;
use bindings::ticknode::plugin::types as wit;
use runtime::Runtime;
pub(crate) use options::OptionStore;
pub use node::WasmNode;
pub use lifecycle::{effective_settings, missing_required};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Loaded plugin: known to the engine, but its types are registered only while enabled.
pub(crate) struct PluginSlot {
    manifest: PluginManifest,
    definitions: Vec<(NodeDefinition, bool)>,
    types: Vec<TypeDefinition>,
    runtime: Arc<Mutex<Runtime>>,
    active: bool,
    /// Result of the last `configure` call (settings error).
    settings_error: Option<UiMessage>,
    /// Validated translation resources of the plugin: `(locale, Fluent source)`.
    translations: Vec<(String, String)>,
}

/// Graph changes after enabling or disabling a plugin.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PluginChange {
    /// Nodes turned into stubs (on disable) or restored to real nodes (on enable).
    pub replaced: Vec<NodeId>,
    /// Connections removed after restoring nodes (when pins changed).
    pub removed: RemovedConnections,
}

#[cfg(test)]
mod tests;
