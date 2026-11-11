//! SDK for TickNode node plugins.
//!
//! A plugin is a WASM component (`crate-type = ["cdylib"]`, target `wasm32-wasip2`) implementing
//! the `wit/plugin.wit` contract. Implement [`Plugin`] and [`PluginNode`], then invoke [`export_plugin!`].
//! Each graph node is a separate [`PluginNode`] instance, so state can be held directly in struct fields.
//!
//! # Language rules
//!
//! Definitions (titles, descriptions, help, pin names, type and setting texts) are written in
//! **English**: they are the fallback shown when no translation exists. Translations are shipped as
//! Fluent `.ftl` files via [`Plugin::TRANSLATIONS`]. **Logs are never translated**: everything passed
//! to [`host::log`] / [`host::warn`] / [`host::error`] and every node error message must be English.
//! Errors shown to the user in the UI (for example from [`Plugin::configure`]) are [`UiError`]s
//! with a translation key and an English fallback.
//!
//! # Translation key conventions
//!
//! Fluent ids may contain only `[a-zA-Z0-9_-]`, so names are slugified: every character outside
//! `[a-z0-9-]` (`/`, `.`, `_`, spaces) becomes `-` and letters are lowercased ([`slug`]).
//!
//! | what | key |
//! |------|-----|
//! | node title / description / help | `node-<type slug>-title`, `-description`, `-help` |
//! | property title / description | `node-<type slug>-prop-<name>-title`, `-description` (the property name as written when it consists of `[A-Za-z0-9_-]`, otherwise its slug) |
//! | select option label | `node-<type slug>-prop-<name>-option-<value slug>` (optional `-short` for MultiSelect chips) |
//! | pin label | `node-<type slug>-pin-<pin name>`: the pin name as written when it consists of `[A-Za-z0-9_-]` (`pin-Val`), otherwise its slug (`Delay ms` -> `pin-delay-ms`) |
//! | category | `category-<category slug>` (the first plugin that defines a category key wins) |
//! | data type | `type-<type name slug>-title`, `-description`, `-field-<field>`, `-value-<enum value>` |
//! | setting | `setting-<plugin slug>-<key slug>-title`, `-description`, `-option-<value>` |
//! | free keys, e.g. `configure` errors | `<plugin slug>-...` (recommended: `<plugin slug>-error-...`; arguments are strings) |
//!
//! The host parses every `.ftl`; a resource with syntax errors is rejected (with a warning in the server
//! log) and messages whose ids do not start with one of the plugin's own prefixes are dropped, so
//! plugins cannot override each other's or the core's texts. Example (`include_str!` embeds the files):
//!
//! ```ignore
//! impl Plugin for MyPlugin {
//!     const TRANSLATIONS: &'static [(&'static str, &'static str)] = &[
//!         ("uk", include_str!("../locales/uk.ftl")),
//!     ];
//! }
//! ```
//!
//! ```ignore
//! use ticknode_plugin_sdk::*;
//!
//! struct MyPlugin;
//! impl Plugin for MyPlugin {
//!     const NODE_TYPES: &'static [&'static str] = &["my/node"];
//!     fn manifest() -> PluginManifest { /* ... */ }
//!     fn create(type_name: &str, node: NodeId) -> Option<Box<dyn PluginNode>> { /* ... */ }
//! }
//! export_plugin!(MyPlugin);
//! ```
//!
//! # Plugin-level connections and events
//!
//! A plugin can own persistent WebSocket connections (capability [`Capability::WebSocket`]):
//! [`host::ws_connect`] / [`host::ws_send`] / [`host::ws_close`] work from any plugin call
//! ([`Plugin::configure`], [`Plugin::tick`], node `execute`/`poll`). The host calls [`Plugin::tick`]
//! once per tick with the connection events ([`WsEvent`]); the plugin answers with [`Emission`]s: outputs
//! pushed to any of its own nodes, as if that node had executed. A node learns its graph id in
//! [`Plugin::create`], so the plugin keeps a `NodeId -> state` registry (in a `thread_local!`) to address
//! emissions. Time can be measured with `std::time::Instant` (WASI clocks).

pub use ticknode_shared::{
    Capability, DataType, DataValue, EnumValue, FieldDefinition, NodeDefinition, NumberValue, ObjectField,
    NodeId, PinDefinition, PinDirection, PinId, PinType, PluginManifest, PropertyDefinition, PropertyKind, PropertyOption, ResourceAction, ResourceCollection, SettingDefinition, SettingKind, TypeDefinition, TypeKind,
    UiMessage, fluent_slug as slug, quantity,
};

/// A translatable error for the user: `UiError::new("my-plugin-error-no-token", "Token is not set")`
/// (key in the plugin's translations, English fallback), optionally with `.arg("name", value)`.
/// `String` / `&str` convert into a fallback-only error.
pub type UiError = UiMessage;

#[doc(hidden)]
pub mod bindings {
    wit_bindgen::generate!({
        path: "../wit",
        world: "plugin",
        pub_export_macro: true,
        export_macro_name: "__export_ticknode_plugin",
        default_bindings_module: "ticknode_plugin_sdk::bindings",
    });
}

mod api;
#[doc(hidden)]
pub use api::{PLUGIN_API_SECTION, PLUGIN_API_SECTION_LEN};

mod events;
mod values;
mod plugin;
mod export;
mod convert;
mod pins;
/// Host services. Tasks (`delay_*`, `http`) can only be started during `execute`/`poll`;
/// their result arrives in `execute` as [`Trigger::Task`].
pub mod host;

pub use events::*;
pub use values::*;
pub use plugin::*;
pub use pins::*;
#[doc(hidden)]
pub use export::{Exporter, NodeHandle};

#[cfg(test)]
mod tests;
