use super::convert::{from_wit_definition, from_wit_manifest, from_wit_type_definition};
use super::effective_settings;
use super::host::Grants;
use super::runtime::{Runtime, engine};
use super::{PluginSlot, bindings};
use crate::TickEngine;
use crate::translations::{AllowedIds, filter_plugin_resource, normalize_locale};
use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use ticknode_shared::*;
use tracing::warn;
use wasmtime::component::{Component, HasSelf, Linker};

/// Validates the translations a plugin ships: normalizes locale codes, rejects resources with syntax
/// errors and drops ids outside the plugin's namespace. Problems are logged (in English) and never fatal.
fn validate_translations(
    plugin: &str,
    raw: Vec<(String, String)>,
    allowed: &AllowedIds,
) -> Vec<(String, String)> {
    let mut valid = Vec::new();
    for (locale, source) in raw {
        let Some(code) = normalize_locale(&locale) else {
            warn!(plugin, locale = %locale, "Ignoring translations with an invalid locale code");
            continue;
        };
        match filter_plugin_resource(plugin, &code, &source, allowed) {
            Ok(filtered) => valid.push((code, filtered)),
            Err(e) => warn!(plugin, locale = %code, "Rejected a translation resource: {e}"),
        }
    }
    valid
}

impl TickEngine {
    /// Loads a plugin component and registers its node types (`add_plugin` + `enable_plugin` with
    /// default settings).
    ///
    /// `grant` decides which capabilities to grant after reading the manifest. The `Storage` capability
    /// gives the plugin the `storage_root/<name>` directory; without `storage_root` it is ignored.
    pub fn load_plugin(
        &mut self,
        wasm: &[u8],
        storage_root: Option<&Path>,
        grant: impl FnOnce(&PluginManifest) -> Vec<Capability>,
    ) -> Result<PluginManifest, UiMessage> {
        let manifest = self.add_plugin(wasm, storage_root, grant)?;
        let settings = effective_settings(&manifest, &[]);
        if let Err(e) = self.enable_plugin(&manifest.name, &settings) {
            self.plugins.remove(&manifest.name);
            return Err(e);
        }
        Ok(manifest)
    }

    /// Compiles a plugin and reads its manifest without enabling it: node types are not registered yet.
    pub fn add_plugin(
        &mut self,
        wasm: &[u8],
        storage_root: Option<&Path>,
        grant: impl FnOnce(&PluginManifest) -> Vec<Capability>,
    ) -> Result<PluginManifest, UiMessage> {
        super::api::check_plugin_api(wasm)?;
        let mut runtime = unprivileged_runtime(wasm)?;
        let (manifest, definitions, types, translations) = read_metadata(&mut runtime)?;
        runtime.instance = None;

        let mut manifest = from_wit_manifest(manifest);
        self.validate_plugin_name(&manifest)?;
        let definitions: Vec<(NodeDefinition, bool)> = definitions
            .into_iter()
            .map(|d| from_wit_definition(d, &manifest.name))
            .collect();
        let types: Vec<TypeDefinition> = types.into_iter().map(from_wit_type_definition).collect();
        self.check_plugin_definitions(&definitions)?;

        runtime.grants = Arc::new(plugin_grants(
            &manifest.name,
            grant(&manifest),
            storage_root,
        )?);

        let node_types: Vec<String> = definitions
            .iter()
            .map(|(d, _)| d.type_name.clone())
            .collect();
        manifest.resources = validate_resources(&manifest.name, manifest.resources, &node_types);
        let type_names: Vec<String> = types.iter().map(|t| t.name.clone()).collect();
        let allowed = AllowedIds::new(&manifest.name, &node_types, &type_names);
        runtime.allowed = allowed.clone();
        let translations = validate_translations(&manifest.name, translations, &allowed);

        self.plugins.insert(
            manifest.name.clone(),
            PluginSlot {
                manifest: manifest.clone(),
                definitions,
                types,
                runtime: Arc::new(Mutex::new(runtime)),
                active: false,
                settings_error: None,
                settings_changed: false,
                translations,
            },
        );
        Ok(manifest)
    }
    fn validate_plugin_name(&self, manifest: &PluginManifest) -> Result<(), UiMessage> {
        if manifest.name.is_empty()
            || !manifest
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(UiMessage::plain(format!(
                "Invalid plugin name: {:?}",
                manifest.name
            )));
        }
        if self.plugins.contains_key(&manifest.name) {
            return Err(UiMessage::plain(format!(
                "Plugin {} is already loaded",
                manifest.name
            )));
        }
        Ok(())
    }

    fn check_plugin_definitions(
        &self,
        definitions: &[(NodeDefinition, bool)],
    ) -> Result<(), UiMessage> {
        for (def, _) in definitions {
            if self.registry.get_definition(&def.type_name).is_some() {
                return Err(UiMessage::plain(format!(
                    "Node type {} is already registered",
                    def.type_name
                )));
            }
        }

        Ok(())
    }
}

fn unprivileged_runtime(wasm: &[u8]) -> Result<Runtime, UiMessage> {
    let component = Component::new(engine(), wasm)
        .map_err(|e| UiMessage::plain(format!("Invalid component: {e:#}")))?;
    let mut linker = Linker::new(engine());
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .map_err(|e| UiMessage::plain(e.to_string()))?;
    bindings::Plugin::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s)
        .map_err(|e| UiMessage::plain(e.to_string()))?;
    let pre = linker
        .instantiate_pre(&component)
        .and_then(bindings::PluginPre::new)
        .map_err(|e| {
            UiMessage::plain(format!("The plugin is incompatible with the host: {e:#}"))
        })?;

    Ok(Runtime {
        pre,
        grants: Arc::default(),
        allowed: AllowedIds::default(),
        instance: None,
        generation: 0,
        settings: None,
        configure_error: None,
        tokio: None,
        tick_backoff_until: 0,
        options: Arc::default(),
    })
}

fn plugin_grants(
    name: &str,
    capabilities: Vec<Capability>,
    storage_root: Option<&Path>,
) -> Result<Grants, UiMessage> {
    let mut grants = Grants {
        name: name.to_string(),
        ..Grants::default()
    };
    for capability in capabilities {
        match capability {
            Capability::Http { allowed_domains } => grants.http_domains.extend(allowed_domains),
            Capability::WebSocket { allowed_hosts } => grants.ws_hosts.extend(allowed_hosts),
            Capability::Storage => {
                if let Some(root) = storage_root {
                    let dir = root.join(name);
                    std::fs::create_dir_all(&dir).map_err(|e| {
                        UiMessage::plain(format!("Plugin directory {}: {e}", dir.display()))
                    })?;
                    grants.storage_dir = Some(dir);
                }
            }
        }
    }
    Ok(grants)
}

type PluginMetadata = (
    super::wit::PluginManifest,
    Vec<super::wit::NodeDefinition>,
    Vec<super::wit::TypeDefinition>,
    Vec<(String, String)>,
);

fn read_metadata(runtime: &mut Runtime) -> Result<PluginMetadata, UiMessage> {
    runtime
        .api()
        .and_then(|(store, api)| {
            Ok((
                api.call_manifest(&mut *store)?,
                api.call_definitions(&mut *store)?,
                api.call_types(&mut *store)?,
                api.call_translations(&mut *store)?,
            ))
        })
        .map_err(|e| UiMessage::plain(format!("Plugin initialization failed: {e:#}")))
}

fn validate_resources(
    plugin: &str,
    resources: Vec<ResourceCollection>,
    node_types: &[String],
) -> Vec<ResourceCollection> {
    let mut valid = Vec::new();
    let mut seen_ids = HashSet::new();
    for mut collection in resources {
        if collection.id.is_empty() {
            warn!(plugin, "Dropping resource collection: empty id");
            continue;
        }
        if !seen_ids.insert(collection.id.clone()) {
            warn!(plugin, id = %collection.id, "Dropping resource collection: duplicate id");
            continue;
        }
        if collection.source.is_empty() {
            warn!(plugin, id = %collection.id, "Dropping resource collection: empty source");
            continue;
        }
        collection.actions.retain(|action| {
            if action.property.is_empty() {
                warn!(plugin, collection = %collection.id, type_name = %action.type_name, "Dropping resource action: empty property");
                return false;
            }
            if !node_types.iter().any(|t| t == &action.type_name) {
                warn!(plugin, collection = %collection.id, type_name = %action.type_name, "Dropping resource action: unknown node type");
                return false;
            }
            true
        });
        if collection.actions.is_empty() {
            warn!(plugin, id = %collection.id, "Dropping resource collection: no valid actions");
            continue;
        }
        valid.push(collection);
    }
    valid
}

#[cfg(test)]
mod tests;
