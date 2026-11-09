//! Pointer gesture state and starting canvas interactions.

use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, PointerEvent};

use crate::clipboard;
use crate::geometry::*;
use crate::state::{Editor, LogKind, Selection, WireKey, WireRoutes};
use super::paste::move_paste;
use super::selection::{SelectBox, nodes_in_box};
use super::wires::WireEnd;

mod motion;
mod decisions;
mod start;
mod points;
pub(crate) use points::edit_route;
pub(crate) use decisions::PointEdit;

use decisions::{Press, Gesture, press, gesture};

/// Pointer movement threshold (px) after which a press is treated as a drag rather than a click.
const DRAG_THRESHOLD: f64 = 3.0;
/// Larger threshold (px) for pulling a point out of a wire, so a click that selects a wire never bends it.
const WIRE_PULL_THRESHOLD: f64 = 8.0;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Drag {
    Idle,
    Pan { start: (f64, f64), origin: Camera, moved: bool },
    /// Dragging selected nodes; `clicked` is the node under the cursor.
    Nodes { origins: Vec<(NodeId, Position)>, routes: WireRoutes, start_world: (f64, f64), start: (f64, f64), moved: bool, clicked: NodeId },
    /// Dragging route point `index`; `origin` is the route being edited, `before` the route to compare on release.
    Point { key: WireKey, index: usize, origin: Vec<Position>, before: Vec<Position>, start_world: (f64, f64), start: (f64, f64), moved: bool },
    /// A press on a wire: pulling it sideways past `WIRE_PULL_THRESHOLD` inserts a point and drags it.
    WirePull { key: WireKey, start: (f64, f64), start_world: (f64, f64) },
    Wire { from: WireEnd, detached: Option<DetachedConnection> },
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
    decisions::wire_selection(
        &el.get_attribute("data-src")?, &el.get_attribute("data-src-pin")?,
        &el.get_attribute("data-dst")?, &el.get_attribute("data-dst-pin")?,
        &el.get_attribute("data-wire")?,
    )
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
    fn send_moves(self, origins: Vec<(NodeId, Position)>, routes: WireRoutes) {
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
        for (key, origin) in routes {
            if !ed.wire_exists_untracked(key) { continue; }
            let points = ed.wire_routes.with_untracked(|all| all.get(&key).cloned().unwrap_or_default());
            if points != origin { commands.push(ClientMessage::SetWireRoute(WireRoute { exec: key.0, connection: key.1, points })); }
        }
        ed.send_batch(commands);
    }

    pub(super) fn pointer_down(self, ev: PointerEvent) {
        let Self { ed, canvas, drag, panning, pending_wire, pointers, .. } = self;
        let p = self.local_point(&ev);
        match press(ed.pending_paste.with_untracked(|p| p.is_some()), &ev.pointer_type(), ev.button()) {
            Press::Place => { move_paste(ed, p); clipboard::place_paste(ed); return; }
            Press::Cancel => { clipboard::cancel_paste(ed); return; }
            Press::Ignore => return,
            Press::Continue => {}
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
            match drag.get_value() {
                Drag::Nodes { origins, routes, moved: true, .. } => self.send_moves(origins, routes),
                Drag::Point { key, before, moved: true, .. } => self.send_point(key, before),
                _ => {}
            }
            pending_wire.set(None);
            drag.set_value(Drag::Pinch {
                start_dist: distance(active[0], active[1]).max(1.0),
                start_mid: ((active[0].0 + active[1].0) / 2.0, (active[0].1 + active[1].1) / 2.0),
                origin: ed.view.get_untracked(),
            });
            return;
        }

        let target = ev.target();
        let detached_hit = closest(target.clone(), "[data-detached]");
        let detached = detached_hit.as_ref().and_then(|el| serde_json::from_str::<DetachedConnection>(&el.get_attribute("data-detached")?).ok())
            .filter(|d| ed.detached.with_untracked(|all| all.contains(d)));
        if ev.button() != 1 && let Some(d) = detached {
            ed.selected_point.set(None);
            ed.selection.set(Some(Selection::Detached(d)));
            if ev.button() == 2 {
                ev.prevent_default();
                ed.wire_menu.set(None);
                ed.picker.set(None);
                ed.wire_context.set(Some(crate::state::WireContext {
                    detached: Some(d), exec: d.exec, connection: d.connection, index: None,
                    world: ed.view.get_untracked().to_world(p.0, p.1),
                    screen: (ev.client_x() as f64, ev.client_y() as f64),
                }));
            } else if detached_hit.is_some_and(|el| el.has_attribute("data-detached-end")) {
                let (from, _) = super::detached::ends(d);
                let live = ed.node(from.node).and_then(|n| n.instance.with_untracked(|i| pin_type(&i.definition, from.pin, from.dir).cloned()));
                if let Some(kind) = live {
                    let from = WireEnd { exec: matches!(kind, PinType::Exec), ..from };
                    ed.wire_menu.set(None);
                    self.cursor.set(ed.view.get_untracked().to_world(p.0, p.1));
                    pending_wire.set(Some(from));
                    drag.set_value(Drag::Wire { from, detached: Some(d) });
                }
            }
            return;
        }
        let pin = closest(target.clone(), "[data-pin]").and_then(|el| parse_pin(&el));
        let point = closest(target.clone(), "[data-wire-point]");
        let index = point.as_ref().and_then(|el| el.get_attribute("data-wire-point")?.parse().ok());
        let wire = point.or_else(|| closest(target.clone(), "[data-wire]")).and_then(|el| parse_wire(&el));
        let node = closest(target, "[data-node]").and_then(|el| el.get_attribute("data-node")).and_then(|s| parse_node_id(&s));
        let view = ed.view.get_untracked();
        let world = view.to_world(p.0, p.1);
        let action = decisions::route_gesture(ev.button(), node, wire.clone(), index)
            .unwrap_or_else(|| gesture(ev.button(), &ev.pointer_type(), pin, wire, node));
        if !matches!(action, Gesture::Point { .. } | Gesture::Context { .. }) { ed.selected_point.set(None); }
        match action {
            Gesture::Pan { moved } => {
                if moved { ev.prevent_default(); }
                drag.set_value(Drag::Pan { start: p, origin: view, moved });
                panning.set(true);
            }
            Gesture::Wire(from) => self.start_wire(from, world),
            Gesture::Point { wire, index } => self.start_point(wire, index, p, world),
            Gesture::Context { wire, index } => {
                ev.prevent_default();
                self.open_context(wire, index, world, (ev.client_x() as f64, ev.client_y() as f64));
            }
            Gesture::Select(selection) => {
                if let Some(key) = points::selection_key(&selection) {
                    drag.set_value(Drag::WirePull { key, start: p, start_world: world });
                }
                ed.selection.set(Some(selection));
            }
            Gesture::Node(id) => self.start_node_click(id, &ev, p, world),
            Gesture::Box => drag.set_value(Drag::Box { start: p, additive: additive(&ev) }),
            Gesture::Ignore => {}
        }
    }
}
