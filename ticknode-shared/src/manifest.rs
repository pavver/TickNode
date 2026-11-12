use crate::SettingDefinition;
use serde::{Deserialize, Serialize};

/// Permissions requested by a plugin in its manifest. Enforced by the host on every plugin call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    /// HTTP(S) to listed domains and their subdomains; `*` — any domain.
    Http { allowed_domains: Vec<String> },
    /// Plugin's own isolated directory (via the WASI filesystem).
    Storage,
    /// Persistent `ws://` / `wss://` connections to listed hosts and their subdomains; `*` — any host.
    WebSocket { allowed_hosts: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceCollection {
    pub id: String,     // unique within the plugin, [a-z0-9_]
    pub title: String,  // English fallback
    pub icon: String,   // "mdi:name", may be empty
    pub source: String, // dynamic options source with the items
    pub actions: Vec<ResourceAction>,
}

impl ResourceCollection {
    pub fn new(id: impl Into<String>, title: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: String::new(),
            source: source.into(),
            actions: Vec::new(),
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn action(mut self, action: ResourceAction) -> Self {
        self.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = ResourceAction>) -> Self {
        self.actions.extend(actions);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceAction {
    pub title: String,                   // English fallback, e.g. "State trigger"
    pub type_name: String,               // node type to create (must belong to this plugin)
    pub property: String,                // string property that receives the item value
    pub requires_source: Option<String>, // offer only for items whose value is also in this options source
}

impl ResourceAction {
    pub fn new(
        title: impl Into<String>,
        type_name: impl Into<String>,
        property: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            type_name: type_name.into(),
            property: property.into(),
            requires_source: None,
        }
    }

    pub fn requires_source(mut self, source: impl Into<String>) -> Self {
        self.requires_source = Some(source.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub requested_capabilities: Vec<Capability>,
    /// Schema of settings configured by the user in the UI (URL, token, etc.).
    pub settings: Vec<SettingDefinition>,
    #[serde(default)]
    pub resources: Vec<ResourceCollection>,
}

impl PluginManifest {
    pub fn new(name: &str, version: &str, author: &str) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            author: author.into(),
            requested_capabilities: Vec::new(),
            settings: Vec::new(),
            resources: Vec::new(),
        }
    }

    pub fn capability(mut self, capability: Capability) -> Self {
        self.requested_capabilities.push(capability);
        self
    }

    pub fn setting(mut self, setting: SettingDefinition) -> Self {
        self.settings.push(setting);
        self
    }

    pub fn resource(mut self, r: ResourceCollection) -> Self {
        self.resources.push(r);
        self
    }
}
