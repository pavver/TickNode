//! WASM plugins: wasmtime components implementing the `wit/plugin.wit` contract.
//!
//! One component instance (`Store`) per plugin; each graph node is a `node` resource within it,
//! so node state persists across calls. Calls are bounded by fuel and memory limits. If a plugin
//! traps (crash, out of fuel), the instance is recreated and nodes are reconstructed from their
//! properties on the next call. The node that caused the failure is halted until a property changes
//! so it does not trap on every tick.

use crate::RemovedConnections;
use std::sync::{Arc, Mutex, MutexGuard};
use ticknode_shared::*;

mod api;
mod bindings;
mod catalog;
mod convert;
mod emissions;
mod host;
mod http;
pub(crate) mod identifiers;
mod lifecycle;
mod loading;
mod node;
mod options;
mod outputs;
mod runtime;

use bindings::exports::ticknode::plugin::nodes::Guest as NodesApi;
use bindings::ticknode::plugin::types as wit;
pub use lifecycle::{effective_settings, missing_required};
pub use node::WasmNode;
pub(crate) use options::OptionStore;
use runtime::Runtime;

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
    /// Synchronous configure error or an asynchronous error reported from plugin-tick.
    settings_error: Option<UiMessage>,
    settings_changed: bool,
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
    pub detached: Vec<DetachedConnection>,
}

#[cfg(test)]
mod tests;
