use ticknode_shared::{PLUGIN_API, UiMessage};
use wasmparser::{Parser, Payload};

const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Checks the SDK's marker, including custom sections inside a component's core modules.
pub(super) fn check_plugin_api(wasm: &[u8]) -> Result<(), UiMessage> {
    let mut found = false;
    for payload in Parser::new(0).parse_all(wasm) {
        let payload = payload.map_err(|e| UiMessage::plain(format!("Invalid component: {e}")))?;
        if let Payload::CustomSection(section) = payload
            && section.name() == "ticknode-plugin-api"
        {
            found = true;
            let (api, sdk) = parse_section(section.data()).ok_or_else(unknown_api)?;
            if api != PLUGIN_API {
                return Err(UiMessage::new(
                    "error-plugin-api-mismatch",
                    format!(
                        "The plugin was built for plugin API {} (SDK {sdk}), but this server uses plugin API {} (TickNode {SERVER_VERSION}). Rebuild the plugin with the current SDK.",
                        &api[..8], &PLUGIN_API[..8],
                    ),
                )
                .arg("plugin_api", &api[..8])
                .arg("plugin_sdk", sdk)
                .arg("server_api", &PLUGIN_API[..8])
                .arg("server_version", SERVER_VERSION));
            }
        }
    }
    if found { Ok(()) } else { Err(unknown_api()) }
}

fn parse_section(data: &[u8]) -> Option<(&str, &str)> {
    let mut parts = std::str::from_utf8(data).ok()?.split_whitespace();
    if parts.next()? != "ticknode-plugin-api/1" {
        return None;
    }
    let api = parts.next()?;
    let sdk = parts.next()?;
    if api.len() != 16
        || !api
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || parts.next().is_some()
    {
        return None;
    }
    Some((api, sdk))
}

fn unknown_api() -> UiMessage {
    UiMessage::new(
        "error-plugin-api-unknown",
        format!(
            "The plugin was built with an SDK that predates plugin API checks. This server uses plugin API {} (TickNode {SERVER_VERSION}). Rebuild the plugin with the current SDK.",
            &PLUGIN_API[..8],
        ),
    )
    .arg("server_api", &PLUGIN_API[..8])
    .arg("server_version", SERVER_VERSION)
}

#[cfg(test)]
mod tests;
