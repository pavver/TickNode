//! Trace presentation and best-effort browser preference storage for the canvas.

use crate::i18n::{self, t};
use crate::panels::{display_value_in, relative_time};
use crate::state::Editor;
use leptos::prelude::*;
use ticknode_shared::*;

const STORAGE_KEY: &str = "ticknode.show-values";

pub fn load_show_values() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
        .as_deref()
        == Some("true")
}

pub fn set_show_values(ed: Editor, show: bool) {
    ed.show_values.set(show);
    // The web-sys bindings catch exceptions from both storage access and writes.
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(STORAGE_KEY, if show { "true" } else { "false" });
    }
}

pub fn toggle_show_values(ed: Editor) {
    set_show_values(ed, !ed.show_values.get_untracked());
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

fn declared_type(node: crate::state::ClientNode, pin: PinId, dir: PinDirection) -> DataType {
    node.instance.with(|i| {
        i.definition
            .pins
            .iter()
            .find(|p| p.id == pin && p.direction == dir)
            .and_then(|p| match &p.pin_type {
                PinType::Data(t) => Some(t.clone()),
                _ => None,
            })
            .unwrap_or(DataType::Any)
    })
}

pub(crate) fn trace_type(ed: Editor, id: NodeId, pin: PinId, dir: PinDirection) -> DataType {
    let declared = ed
        .node_tracked(id)
        .map(|node| declared_type(node, pin, dir))
        .unwrap_or(DataType::Any);
    if declared == DataType::Any && dir == PinDirection::Input {
        ed.source_type(id, pin).unwrap_or(declared)
    } else {
        declared
    }
}

pub fn pin_status(ed: Editor, id: NodeId, pin: PinId, dir: PinDirection, exec: bool) -> String {
    let Some(node) = ed.node_tracked(id) else {
        return String::new();
    };
    node.trace.with(|trace| {
        let age = || relative_time(ed.now.get() - node.last_exec.get());
        if exec {
            if dir == PinDirection::Output && trace.as_ref().is_some_and(|t| t.fired.contains(&pin))
            {
                t!("pin-fired-ago", time = age())
            } else {
                String::new()
            }
        } else {
            match trace.as_ref().and_then(|t| trace_value(t, pin, dir)) {
                Some(value) => format!(
                    "{}\n{}",
                    truncate(display_value_in(value, &trace_type(ed, id, pin, dir)), 200),
                    age()
                ),
                None => t!("pin-no-value"),
            }
        }
    })
}

/// A native tooltip disappears whenever its text changes, and status texts age every second, so the text is
/// captured once when the pointer enters (`title` on HTML elements, the child `<title>` on SVG groups).
pub fn capture_title(ev: &web_sys::PointerEvent, text: impl Fn() -> String) {
    use wasm_bindgen::JsCast;
    let Some(el) = ev
        .current_target()
        .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
    else {
        return;
    };
    let text = untrack(text);
    if el.namespace_uri().as_deref() == Some("http://www.w3.org/2000/svg") {
        if let Ok(Some(title)) = el.query_selector(":scope > title") {
            title.set_text_content(Some(&text));
        }
    } else {
        let _ = el.set_attribute("title", &text);
    }
}

pub fn wire_value(ed: Editor, conn: ConnectionModel) -> Option<String> {
    let node = ed.node_tracked(conn.src_node)?;
    node.trace.with(|trace| {
        Some(truncate(
            i18n::value_as_text(
                trace_value(trace.as_ref()?, conn.src_pin, PinDirection::Output)?,
                &declared_type(node, conn.src_pin, PinDirection::Output),
            ),
            24,
        ))
    })
}

pub fn error_text(ed: Editor, id: NodeId) -> String {
    ed.node_errors.with(|errors| {
        let Some(entry) = errors.get(&id) else {
            return String::new();
        };
        let repeated = if entry.error.count > 1 {
            format!("\n×{}", entry.error.count)
        } else {
            String::new()
        };
        format!(
            "{}{repeated}\n{}",
            i18n::ui_text(&entry.error.message),
            relative_time(entry.age_ms(ed.now.get()))
        )
    })
}

#[cfg(test)]
mod tests;
