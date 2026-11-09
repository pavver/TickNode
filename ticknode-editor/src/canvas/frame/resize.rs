//! Pointer-captured frame resizing; only the completed gesture is sent to the server.

use leptos::{ev, html, prelude::*};
use ticknode_shared::{ClientMessage, FrameId, NodeSize};

use super::resize_frame_size;
use crate::i18n::t;
use crate::state::use_editor;

#[derive(Clone, Copy)]
struct FrameResizeDrag {
    pointer: i32,
    start: (f64, f64),
    size: NodeSize,
    zoom: f64,
}

#[component]
pub fn FrameResizeHandle(id: FrameId) -> impl IntoView {
    let ed = use_editor();
    let handle = NodeRef::<html::Button>::new();
    let drag = StoredValue::new(None::<FrameResizeDrag>);

    let cancel = move || {
        if let Some(active) = drag.get_value() {
            drag.set_value(None);
            ed.frame_size_previews.update(|p| {
                p.remove(&id);
            });
            if let Some(el) = handle.get_untracked() {
                let _ = el.release_pointer_capture(active.pointer);
            }
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
            class="canvas-frame-resize"
            node_ref=handle
            title=move || t!("frame-resize-title")
            aria-label=move || t!("frame-resize-title")
            on:pointerdown=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                if ev.button() != 0 || drag.get_value().is_some() {
                    return;
                }
                ev.prevent_default();
                let Some(frame) = untrack(|| ed.frames.with(|all| all.get(&id).cloned())) else {
                    return;
                };
                drag.set_value(Some(FrameResizeDrag {
                    pointer: ev.pointer_id(),
                    start: (ev.client_x() as f64, ev.client_y() as f64),
                    size: frame.size,
                    zoom: ed.view.get_untracked().zoom,
                }));
                if let Some(el) = handle.get_untracked() {
                    let _ = el.focus();
                    let _ = el.set_pointer_capture(ev.pointer_id());
                }
            }
            on:pointermove=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                let Some(active) = drag.get_value().filter(|a| a.pointer == ev.pointer_id()) else {
                    return;
                };
                let size = resize_frame_size(
                    active.size,
                    (
                        (ev.client_x() as f64 - active.start.0) / active.zoom,
                        (ev.client_y() as f64 - active.start.1) / active.zoom,
                    ),
                );
                ed.frame_size_previews.update(|p| {
                    p.insert(id, size);
                });
            }
            on:pointerup=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                let Some(active) = drag.get_value().filter(|a| a.pointer == ev.pointer_id()) else {
                    return;
                };
                let current_size = ed.frame_size_previews.with_untracked(|p| p.get(&id).copied()).unwrap_or(active.size);
                if current_size != active.size {
                    // Keep the preview until `FrameSet` arrives, so the frame does not jump back.
                    drag.set_value(None);
                    if let Some(el) = handle.get_untracked() {
                        let _ = el.release_pointer_capture(active.pointer);
                    }
                    if let Some(mut frame) = ed.frames.with_untracked(|all| all.get(&id).cloned()) {
                        frame.size = current_size;
                        ed.send(ClientMessage::SetFrame(frame));
                    }
                } else {
                    cancel();
                }
            }
            on:pointercancel=move |ev: web_sys::PointerEvent| {
                ev.stop_propagation();
                cancel();
            }
            on:lostpointercapture=move |_| cancel()
        />
    }
}
