use super::{PluginChange, WasmNode, lock};
use crate::{RemovedConnections, TickEngine};
use ticknode_shared::*;

/// Effective settings for a plugin: stored values, falling back to manifest defaults for missing ones.
/// An empty string is treated as unset.
pub fn effective_settings(
    manifest: &PluginManifest,
    stored: &[(String, DataValue)],
) -> Vec<(String, DataValue)> {
    manifest
        .settings
        .iter()
        .filter_map(|def| {
            let value = stored
                .iter()
                .find(|(k, v)| *k == def.key && !matches!(v, DataValue::String(s) if s.is_empty()))
                .map(|(_, v)| v.clone());
            Some((def.key.clone(), value.or_else(|| def.default.clone())?))
        })
        .collect()
}

/// First required setting that has no value.
pub fn missing_required(
    manifest: &PluginManifest,
    effective: &[(String, DataValue)],
) -> Option<UiMessage> {
    manifest
        .settings
        .iter()
        .find(|d| d.required && !effective.iter().any(|(k, _)| *k == d.key))
        .map(|d| {
            UiMessage::new(
                "error-setting-required",
                format!("Required setting \"{}\" is not set", d.title),
            )
            .arg("setting", &d.title)
            .arg("plugin", &manifest.name)
            .arg("key", &d.key)
        })
}

/// A plugin crash reported to the user.
pub(super) fn crash_message(reason: &str) -> UiMessage {
    UiMessage::new("error-plugin-crash", format!("Plugin crashed: {reason}")).arg("reason", reason)
}

fn plugin_not_loaded(name: &str) -> UiMessage {
    UiMessage::new(
        "error-plugin-not-loaded",
        format!("Plugin {name} is not loaded"),
    )
    .arg("plugin", name)
}

fn named_types(t: &DataType, out: &mut Vec<String>) {
    match t {
        DataType::Named(name) => out.push(name.clone()),
        DataType::List(inner) => named_types(inner, out),
        _ => {}
    }
}

impl TickEngine {
    /// Settings error from configure or plugin-tick; the plugin remains loaded.
    pub fn plugin_settings_error(&self, name: &str) -> Option<UiMessage> {
        self.plugins.get(name)?.settings_error.clone()
    }

    /// Plugin names whose settings error changed asynchronously during plugin-tick.
    pub fn take_plugin_settings_changes(&mut self) -> Vec<String> {
        self.plugins
            .iter_mut()
            .filter_map(|(name, slot)| {
                std::mem::take(&mut slot.settings_changed).then(|| name.clone())
            })
            .collect()
    }

    /// Enables a plugin: registers its types, applies settings (`settings` already includes defaults,
    /// see `effective_settings`), and revives stubs of its types into real nodes.
    pub fn enable_plugin(
        &mut self,
        name: &str,
        settings: &[(String, DataValue)],
    ) -> Result<PluginChange, UiMessage> {
        let slot = self
            .plugins
            .get(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        if slot.active {
            return Err(UiMessage::plain(format!(
                "Plugin {name} is already enabled"
            )));
        }
        let (definitions, types, runtime) = (
            slot.definitions.clone(),
            slot.types.clone(),
            slot.runtime.clone(),
        );
        for (def, _, _) in &definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(UiMessage::plain(format!(
                    "Node type {} is already registered",
                    def.type_name
                )));
            }
        }
        check_reserved_types(name, &types)?;
        let node_definitions: Vec<NodeDefinition> =
            definitions.iter().map(|(d, _, _)| d.clone()).collect();
        self.register_types_owned(Some(name), &types, &node_definitions)
            .map_err(|e| UiMessage::plain(format!("Plugin {name}: {e}")))?;
        self.clear_definitions();
        for (def, polls, pure) in definitions {
            let runtime = runtime.clone();
            self.registry.register_owned(
                name,
                Box::new(move |id| {
                    Box::new(WasmNode::new(id, def.clone(), polls, pure, runtime.clone()))
                }),
            );
        }

        if let Some(namespace) = &self.plugins[name].manifest.values_namespace {
            self.plugin_values
                .catalogs
                .entry(namespace.clone())
                .or_default();
        }
        self.refresh_all_value_usage();
        self.prepare_value_watches();
        let settings_error = self.apply_settings(name, settings);
        let slot = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        slot.active = true;
        slot.settings_error = settings_error;

        self.sync_plugin_values();
        let (replaced, removed, detached) = self.revive_stubs();
        Ok(PluginChange {
            replaced,
            removed,
            detached,
        })
    }

    /// Disables a plugin: its nodes become stubs (connections and properties are preserved), node
    /// and data types are unregistered, and the plugin instance is dropped. Rejected if another
    /// enabled plugin uses this plugin's data types.
    pub fn disable_plugin(&mut self, name: &str) -> Result<PluginChange, UiMessage> {
        let slot = self
            .plugins
            .get(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        if !slot.active {
            return Err(UiMessage::plain(format!(
                "Plugin {name} is already disabled"
            )));
        }
        let owned: Vec<&String> = self
            .type_owners
            .iter()
            .filter(|(_, o)| *o == name)
            .map(|(t, _)| t)
            .collect();
        let dependents: Vec<&str> = self
            .plugins
            .iter()
            .filter(|(other, p)| other.as_str() != name && p.active)
            .filter(|(_, p)| uses_owned_types(p, &owned))
            .map(|(other, _)| other.as_str())
            .collect();
        if !dependents.is_empty() {
            let list = dependents
                .iter()
                .map(|d| format!("\"{d}\""))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(UiMessage::new(
                "error-plugin-disable-blocked",
                format!("Plugin \"{name}\" cannot be disabled: its data types are used by {list}; disable {} first", if dependents.len() == 1 { "it" } else { "them" }),
            ).arg("plugin", name).arg("dependents", list));
        }

        let node_types: std::collections::HashSet<String> =
            self.registry.types_owned_by(name).into_iter().collect();
        self.clear_definitions();
        let replaced = self.replace_with_stubs(&node_types, name);
        self.registry.unregister_owner(name);
        let type_names: Vec<String> = self
            .type_owners
            .iter()
            .filter(|(_, o)| *o == name)
            .map(|(t, _)| t.clone())
            .collect();
        for t in type_names {
            self.type_owners.remove(&t);
            self.types.remove(&t);
        }
        self.refresh_identifier_labels();
        self.notify_types_changed();
        let slot = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        slot.active = false;
        slot.settings_error = None;
        // Node resources were already dropped when replaced with stubs; the instance is no longer needed
        {
            let mut rt = lock(&slot.runtime);
            rt.discard();
            lock(&rt.options).clear();
        }
        let namespace = slot.manifest.values_namespace.clone();
        {
            let rt = lock(&slot.runtime);
            let mut store = lock(&rt.values);
            store.catalog = Default::default();
            store.pending.clear();
            store.watched.clear();
        }
        if let Some(namespace) = namespace {
            self.remove_values_namespace(&namespace);
        }
        Ok(PluginChange {
            replaced,
            removed: RemovedConnections::default(),
            detached: Vec::new(),
        })
    }

    /// Passes new settings to the plugin (`settings` already includes defaults). Does nothing for a
    /// disabled plugin: settings will be applied when enabled. `Err` is the plugin's response, which
    /// is also recorded as the settings error.
    pub fn configure_plugin(
        &mut self,
        name: &str,
        settings: &[(String, DataValue)],
    ) -> Result<(), UiMessage> {
        let slot = self
            .plugins
            .get(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        if !slot.active {
            return Ok(());
        }
        let ids: Vec<_> = self
            .node_ids()
            .into_iter()
            .filter(|id| {
                self.node_definition(*id)
                    .is_some_and(|d| self.registry.owner_of(&d.type_name) == Some(name))
            })
            .collect();
        for id in ids {
            self.clear_node_error(id);
        }
        self.prepare_value_watches();
        let error = self.apply_settings(name, settings);
        self.sync_plugin_values();
        let slot = self
            .plugins
            .get_mut(name)
            .ok_or_else(|| plugin_not_loaded(name))?;
        slot.settings_error = error.clone();
        error.map_or(Ok(()), Err)
    }

    fn apply_settings(&self, name: &str, settings: &[(String, DataValue)]) -> Option<UiMessage> {
        let slot = &self.plugins[name];
        lock(&slot.runtime)
            .configure(
                settings,
                self.host_state.runtime.clone(),
                self.host_state.blob_store.clone(),
            )
            .err()
            .or_else(|| missing_required(&slot.manifest, settings))
    }
}

fn uses_owned_types(plugin: &super::PluginSlot, owned: &[&String]) -> bool {
    let mut used = Vec::new();
    for (def, _, _) in &plugin.definitions {
        for pin in &def.pins {
            if let PinType::Data(t) = &pin.pin_type {
                named_types(t, &mut used);
            }
        }
    }
    for t in &plugin.types {
        for f in t.fields() {
            named_types(&f.data_type, &mut used);
        }
    }
    used.iter().any(|u| owned.contains(&u))
}

/// Keep the reserved-type rejection translatable at the plugin boundary.
fn check_reserved_types(plugin: &str, types: &[TypeDefinition]) -> Result<(), UiMessage> {
    if let Some(def) = types
        .iter()
        .find(|d| d.name.starts_with("quantity.") || d.quantity().is_some())
    {
        return Err(UiMessage::new(
            "error-plugin-quantity-type",
            format!(
                "Plugin {plugin} cannot declare reserved quantity type {}",
                def.name
            ),
        )
        .arg("plugin", plugin)
        .arg("type", &def.name));
    }
    if let Some(def) = types.iter().find(|d| d.name.starts_with("media.")) {
        return Err(UiMessage::new(
            "error-plugin-media-type",
            format!(
                "Plugin {plugin} cannot declare reserved media type {}",
                def.name
            ),
        )
        .arg("plugin", plugin)
        .arg("type", &def.name));
    }
    Ok(())
}

#[cfg(test)]
mod reserved_type_tests {
    use super::*;

    #[test]
    fn reserved_quantity_registration_returns_a_translatable_plugin_error() {
        for definition in [
            TypeDefinition::new_enum("quantity.fake", "Fake", &[("x", "X")]),
            quantity::builtin_quantities().remove(0),
        ] {
            let error =
                check_reserved_types("example", std::slice::from_ref(&definition)).unwrap_err();
            assert_eq!(error.key, "error-plugin-quantity-type");
            assert!(error.args.contains(&("type".into(), definition.name)));
        }
        assert!(
            check_reserved_types(
                "example",
                &[TypeDefinition::new_enum(
                    "example.mode",
                    "Mode",
                    &[("x", "X")]
                )]
            )
            .is_ok()
        );
    }

    #[test]
    fn reserved_media_registration_returns_a_translatable_plugin_error() {
        let definition = TypeDefinition::new_enum("media.fake", "Fake", &[("x", "X")]);
        let error = check_reserved_types("example", std::slice::from_ref(&definition)).unwrap_err();
        assert_eq!(error.key, "error-plugin-media-type");
        assert!(error.args.contains(&("type".into(), definition.name)));
    }
}
