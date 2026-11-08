use serde::{Deserialize, Serialize};
use crate::SettingDefinition;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub author: String,
    pub requested_capabilities: Vec<Capability>,
    /// Schema of settings configured by the user in the UI (URL, token, etc.).
    pub settings: Vec<SettingDefinition>,
}

impl PluginManifest {
    pub fn new(name: &str, version: &str, author: &str) -> Self {
        Self { name: name.into(), version: version.into(), author: author.into(), requested_capabilities: Vec::new(), settings: Vec::new() }
    }

    pub fn capability(mut self, capability: Capability) -> Self {
        self.requested_capabilities.push(capability);
        self
    }

    pub fn setting(mut self, setting: SettingDefinition) -> Self {
        self.settings.push(setting);
        self
    }
}
