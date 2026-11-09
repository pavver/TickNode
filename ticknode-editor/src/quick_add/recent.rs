//! Best-effort browser storage; web-sys catches storage access exceptions.

use crate::state::Editor;
use leptos::prelude::*;
use super::catalog::update_recent;

const STORAGE_KEY: &str = "ticknode.recent-nodes";

pub fn load() -> Vec<String> {
    let stored: Vec<String> = window().local_storage().ok().flatten()
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    // Normalize old or manually edited storage without changing its newest-first order.
    stored.iter().rev().fold(Vec::new(), |list, name| update_recent(&list, name))
}

pub fn record(ed: Editor, type_name: &str) {
    let recent = ed.recent_nodes.with_untracked(|list| update_recent(list, type_name));
    if let (Some(storage), Ok(json)) = (window().local_storage().ok().flatten(), serde_json::to_string(&recent)) {
        let _ = storage.set_item(STORAGE_KEY, &json);
    }
    ed.recent_nodes.set(recent);
}
