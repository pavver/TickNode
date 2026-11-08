//! Pointer gesture state and starting canvas interactions.

use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};

use crate::clipboard;
use crate::geometry::*;
use crate::state::{Editor, LogKind, Selection};
use super::paste::move_paste;
use super::selection::{SelectBox, nodes_in_box};
use super::wires::WireEnd;

mod motion;

/// Pointer movement threshold (px) after which a press is treated as a drag rather than a click.
const DRAG_THRESHOLD: f64 = 3.0;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Drag {
    Idle,
    Pan { start: (f64, f64), origin: Camera, moved: bool },
    /// Dragging selected nodes; `clicked` is the node under the cursor.
    Nodes { origins: Vec<(NodeId, Position)>, start_world: (f64, f64), start: (f64, f64), moved: bool, clicked: NodeId },
    Wire { from: WireEnd },
    Pinch { start_dist: f64, start_mid: (f64, f64), origin: Camera },
    /// Selection marquee (canvas screen coordinates). `additive` is Shift/Ctrl: add to selection.
    Box { start: (f64, f64), additive: bool },
}

#[derive(Clone, Copy)]
pub(super) struct Interaction {
    pub(super) ed: Editor,
    pub(super) canvas: NodeRef<html::Div>,
    pub(super) drag: StoredValue<Drag>,
    pub(super) panning: RwSignal<bool>,
    pub(super) cursor: RwSignal<(f64, f64)>,
    pub(super) pending_wire: RwSignal<Option<WireEnd>>,
    pub(super) select_box: RwSignal<Option<SelectBox>>,
    pub(super) pointers: StoredValue<HashMap<i32, (f64, f64)>>,
}

fn additive(ev: &PointerEvent) -> bool {
    ev.shift_key() || ev.ctrl_key() || ev.meta_key()
}

fn parse_node_id(s: &str) -> Option<NodeId> {
    uuid::Uuid::parse_str(s).ok().map(NodeId)
}

fn closest(target: Option<web_sys::EventTarget>, selector: &str) -> Option<Element> {
    target?.dyn_into::<Element>().ok()?.closest(selector).ok().flatten()
}

fn parse_pin(el: &Element) -> Option<WireEnd> {
    Some(WireEnd {
        node: parse_node_id(&el.get_attribute("data-node")?)?,
        pin: PinId(el.get_attribute("data-pin")?.parse().ok()?),
        dir: if el.get_attribute("data-dir")? == "in" { PinDirection::Input } else { PinDirection::Output },
        exec: el.get_attribute("data-kind")? == "exec",
    })
}

fn parse_wire(el: &Element) -> Option<Selection> {
    let c = ConnectionModel {
        src_node: parse_node_id(&el.get_attribute("data-src")?)?,
        src_pin: PinId(el.get_attribute("data-src-pin")?.parse().ok()?),
        dst_node: parse_node_id(&el.get_attribute("data-dst")?)?,
        dst_pin: PinId(el.get_attribute("data-dst-pin")?.parse().ok()?),
    };
    Some(if el.get_attribute("data-wire")? == "exec" { Selection::Exec(c) } else { Selection::Data(c) })
}

fn distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Interaction {
    fn local_point(self, ev: &PointerEvent) -> (f64, f64) {
        let canvas = self.canvas;
        canvas
            .get_untracked()
            .map(|c| {
                let r = c.get_bounding_client_rect();
                (ev.client_x() as f64 - r.left(), ev.client_y() as f64 - r.top())
            })
            .unwrap_or((0.0, 0.0))
    }

    // Send new positions of dragged nodes to the server. Positions were already updated locally, so an
    // interrupted drag (pointercancel, second finger) is also sent to keep editor and server in sync.
    fn send_moves(self, origins: Vec<(NodeId, Position)>) {
        let ed = self.ed;
        let mut commands = Vec::new();
        for (id, origin) in origins {
            if let Some(node) = ed.node(id) {
                let position = node.pos.get_untracked();
                if position != origin {
                    commands.push(ClientMessage::MoveNode { id, position });
                }
            }
        }
        ed.send_batch(commands);
    }

    pub(super) fn pointer_down(self, ev: PointerEvent) {
        let Self { ed, canvas, drag, panning, cursor, pending_wire, pointers, .. } = self;
        let p = self.local_point(&ev);
        // Paste placement: LMB (or tap) places, RMB cancels, middle button pans as usual
        if ed.pending_paste.with_untracked(|p| p.is_some()) && ev.button() != 1 {
            match ev.button() {
                0 => {
                    move_paste(ed, p);
                    clipboard::place_paste(ed);
                }
                2 => { clipboard::cancel_paste(ed); }
                _ => {}
            }
            return;
        }
        if ev.pointer_type() == "mouse" && ev.button() != 0 && ev.button() != 1 {
            return;
        }
        pointers.update_value(|ps| {
            ps.insert(ev.pointer_id(), p);
        });
        if let Some(c) = canvas.get_untracked() {
            let _ = c.set_pointer_capture(ev.pointer_id());
        }

        // Second finger starts pinch-to-zoom
        let active: Vec<(f64, f64)> = pointers.with_value(|ps| ps.values().copied().collect());
        if active.len() == 2 {
            if let Drag::Nodes { origins, moved: true, .. } = drag.get_value() {
                self.send_moves(origins);
            }
            pending_wire.set(None);
            drag.set_value(Drag::Pinch {
                start_dist: distance(active[0], active[1]).max(1.0),
                start_mid: ((active[0].0 + active[1].0) / 2.0, (active[0].1 + active[1].1) / 2.0),
                origin: ed.view.get_untracked(),
            });
            return;
        }

        let view = ed.view.get_untracked();
        let world = view.to_world(p.0, p.1);
        let target = ev.target();
        if ev.button() == 1 {
            // Middle button pans the canvas (CAD-style), regardless of where the press started
            ev.prevent_default();
            drag.set_value(Drag::Pan { start: p, origin: view, moved: true });
            panning.set(true);
        } else if let Some(end) = closest(target.clone(), "[data-pin]").and_then(|el| parse_pin(&el)) {
            let mut from = end;
            // Dragging from a connected data input disconnects and relocates the existing wire
            if end.dir == PinDirection::Input && !end.exec {
                let existing = ed.data.with_untracked(|d| d.iter().find(|c| c.dst_node == end.node && c.dst_pin == end.pin).copied());
                if let Some(c) = existing {
                    ed.send(ClientMessage::DisconnectData { dst_node: c.dst_node, dst_pin: c.dst_pin });
                    from = WireEnd { node: c.src_node, pin: c.src_pin, dir: PinDirection::Output, exec: false };
                }
            }
            cursor.set(world);
            pending_wire.set(Some(from));
            drag.set_value(Drag::Wire { from });
        } else if let Some(sel) = closest(target.clone(), "[data-wire]").and_then(|el| parse_wire(&el)) {
            ed.selection.set(Some(sel));
        } else if let Some(id) = closest(target, "[data-node]").and_then(|el| el.get_attribute("data-node")).and_then(|s| parse_node_id(&s)) {
            if additive(&ev) {
                // Shift/Ctrl+click toggles the node in the selection
                let mut ids = ed.selected_nodes();
                match ids.iter().position(|n| *n == id) {
                    Some(i) => { ids.remove(i); }
                    None => ids.push(id),
                }
                ed.select_nodes(ids);
                return;
            }
            if !ed.selection.with_untracked(|s| matches!(s, Some(Selection::Nodes(set)) if set.contains(&id))) {
                ed.select_nodes([id]);
            }
            let origins = ed.selected_nodes().into_iter()
                .filter_map(|n| ed.node(n).map(|node| (n, node.pos.get_untracked())))
                .collect();
            drag.set_value(Drag::Nodes { origins, start_world: world, start: p, moved: false, clicked: id });
        } else if ev.pointer_type() == "touch" {
            // On touch screens, dragging the background with one finger pans the canvas
            drag.set_value(Drag::Pan { start: p, origin: view, moved: false });
            panning.set(true);
        } else {
            drag.set_value(Drag::Box { start: p, additive: additive(&ev) });
        }
    }
}
