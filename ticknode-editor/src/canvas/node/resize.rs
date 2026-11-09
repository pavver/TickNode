//! Pointer-captured node resizing; only the completed gesture is sent to the server.
use leptos::{ev, html, prelude::*};
use ticknode_shared::{ClientMessage, NodeId, NodeSize};
use crate::geometry::resize_size;
use crate::i18n::t;
use crate::state::use_editor;

#[derive(Clone, Copy)]
struct ResizeDrag {
    pointer: i32,
    start: (f64, f64),
    size: NodeSize,
    zoom: f64,
}

#[component]
pub(super) fn ResizeHandle(id: NodeId) -> impl IntoView {
    let ed = use_editor();
    let handle = NodeRef::<html::Button>::new();
    let drag = StoredValue::new(None::<ResizeDrag>);
    let cancel = move || {
        if let Some(active) = drag.get_value() {
            drag.set_value(None);
            if let Some(node) = ed.node(id) { node.size_preview.set(None); }
            if let Some(el) = handle.get_untracked() { let _ = el.release_pointer_capture(active.pointer); }
        }
    };
    let keys = window_event_listener(ev::keydown, move |ev| {
        if ev.key() == "Escape" && drag.get_value().is_some() {
            ev.prevent_default();
            cancel();
        }
    });
    on_cleanup(move || keys.remove());
    view! {
        <button
            class="node-resize"
            node_ref=handle
            title=move || t!("node-resize-title")
            aria-label=move || t!("node-resize-title")
            on:pointerdown=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                if ev.button() != 0 || drag.get_value().is_some() { return; }
                ev.prevent_default();
                let Some(layout) = untrack(|| ed.layout(id)) else { return };
                drag.set_value(Some(ResizeDrag {
                    pointer: ev.pointer_id(), start: (ev.client_x() as f64, ev.client_y() as f64),
                    size: NodeSize { width: layout.width, height: layout.height }, zoom: ed.view.get_untracked().zoom,
                }));
                if let Some(el) = handle.get_untracked() {
                    let _ = el.focus();
                    let _ = el.set_pointer_capture(ev.pointer_id());
                }
            }
            on:pointermove=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                let Some(active) = drag.get_value().filter(|a| a.pointer == ev.pointer_id()) else { return };
                let Some(layout) = untrack(|| ed.layout(id)) else { cancel(); return };
                let size = resize_size(active.size, (
                    (ev.client_x() as f64 - active.start.0) / active.zoom,
                    (ev.client_y() as f64 - active.start.1) / active.zoom,
                ), layout.natural_height);
                if let Some(node) = ed.node(id) { node.size_preview.set(Some(size)); }
            }
            on:pointerup=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                let Some(active) = drag.get_value().filter(|a| a.pointer == ev.pointer_id()) else { return };
                let size = ed.node(id).and_then(|node| node.size_preview.get_untracked());
                match size.filter(|size| *size != active.size) {
                    Some(size) => {
                        // Keep the preview until `NodeResized` arrives, so the node does not jump back.
                        drag.set_value(None);
                        if let Some(el) = handle.get_untracked() { let _ = el.release_pointer_capture(active.pointer); }
                        ed.send(ClientMessage::ResizeNode { id, size: Some(size) });
                    }
                    None => cancel(),
                }
            }
            on:pointercancel=move |ev: web_sys::PointerEvent| { ev.stop_propagation(); cancel(); }
            on:lostpointercapture=move |_| cancel()
            on:dblclick=move |ev| {
                ev.stop_propagation();
                ev.prevent_default();
                cancel();
                ed.send(ClientMessage::ResizeNode { id, size: None });
            }
        ></button>
    }
}
