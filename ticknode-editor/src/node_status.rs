//! Trace presentation and best-effort browser preference storage for the canvas.

use leptos::prelude::*;
use ticknode_shared::*;
use crate::i18n::{self, t};
use crate::panels::{display_value, relative_time};
use crate::state::Editor;

const STORAGE_KEY: &str = "ticknode.show-values";

pub fn load_show_values() -> bool {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten()).as_deref() == Some("true")
}

pub fn toggle_show_values(ed: Editor) {
    let show = !ed.show_values.get_untracked();
    ed.show_values.set(show);
    // The web-sys bindings catch exceptions from both storage access and writes.
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, if show { "true" } else { "false" });
    }
}

pub fn trace_value(trace: &NodeTrace, pin: PinId, dir: PinDirection) -> Option<&DataValue> {
    match dir {
        PinDirection::Input => trace.inputs.iter().find(|(id, _)| *id == pin)?.1.as_ref(),
        PinDirection::Output => Some(&trace.outputs.iter().find(|(id, _)| *id == pin)?.1),
    }
}

pub fn truncate(text: String, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text,
    }
}

pub fn pin_status(ed: Editor, id: NodeId, pin: PinId, dir: PinDirection, exec: bool) -> String {
    let Some(node) = ed.node_tracked(id) else { return String::new() };
    node.trace.with(|trace| {
        let age = || relative_time(ed.now.get() - node.last_exec.get());
        if exec {
            if dir == PinDirection::Output && trace.as_ref().is_some_and(|t| t.fired.contains(&pin)) {
                t!("pin-fired-ago", time = age())
            } else { String::new() }
        } else {
            match trace.as_ref().and_then(|t| trace_value(t, pin, dir)) {
                Some(value) => format!("{}\n{}", truncate(display_value(value), 200), age()),
                None => t!("pin-no-value"),
            }
        }
    })
}

pub fn wire_value(ed: Editor, conn: ConnectionModel) -> Option<String> {
    let node = ed.node_tracked(conn.src_node)?;
    node.trace.with(|trace| {
        Some(truncate(display_value(trace_value(trace.as_ref()?, conn.src_pin, PinDirection::Output)?), 24))
    })
}

pub fn error_text(ed: Editor, id: NodeId) -> String {
    ed.node_errors.with(|errors| {
        let Some(entry) = errors.get(&id) else { return String::new() };
        let repeated = if entry.error.count > 1 { format!("\n×{}", entry.error.count) } else { String::new() };
        format!("{}{repeated}\n{}", i18n::ui_text(&entry.error.message), relative_time(entry.age_ms(ed.now.get())))
    })
}

#[cfg(test)]
mod tests;
