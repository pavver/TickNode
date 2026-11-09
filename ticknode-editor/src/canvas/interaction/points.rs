//! Wire point gestures and commands shared with the context menu.

use super::*;
use crate::state::{WireContext, WireKey};
use super::decisions::PointEdit;

pub(crate) fn selection_key(selection: &Selection) -> Option<WireKey> {
    match selection { Selection::Exec(c) => Some((true, *c)), Selection::Data(c) => Some((false, *c)), _ => None }
}

pub(crate) fn edit_route(ed: Editor, key: WireKey, world: (f64, f64), edit: PointEdit) {
    if !ed.wire_exists_untracked(key) { return; }
    let (_, c) = key;
    let Some((a, b)) = untrack(|| {
        let a = super::super::wires::anchor(ed, c.src_node, c.src_pin, PinDirection::Output)?;
        let b = super::super::wires::anchor(ed, c.dst_node, c.dst_pin, PinDirection::Input)?;
        Some((a, b))
    }) else { return };
    let points = ed.wire_routes.with_untracked(|all| all.get(&key).cloned().unwrap_or_default());
    if let Some(points) = decisions::edit_points(&points, a, b, world, edit) {
        ed.send(ClientMessage::SetWireRoute(WireRoute { exec: key.0, connection: key.1, points }));
    }
}

impl Interaction {
    pub(super) fn start_point(self, wire: Selection, index: usize, start: (f64, f64), start_world: (f64, f64)) {
        let Some(key) = selection_key(&wire) else { return };
        let origin = self.ed.wire_routes.with_untracked(|all| all.get(&key).cloned().unwrap_or_default());
        if index >= origin.len() { return; }
        self.ed.selection.set(Some(wire));
        self.ed.selected_point.set(Some((key, index)));
        self.drag.set_value(Drag::Point { key, index, origin, start_world, start, moved: false });
    }

    pub(super) fn open_context(self, wire: Selection, index: Option<usize>, world: (f64, f64), screen: (f64, f64)) {
        let Some((exec, connection)) = selection_key(&wire) else { return };
        self.ed.selection.set(Some(wire));
        self.ed.picker.set(None);
        self.ed.wire_menu.set(None);
        self.ed.wire_context.set(Some(WireContext { exec, connection, index, world, screen }));
    }

    pub(in crate::canvas) fn double_click(self, ev: web_sys::MouseEvent) {
        // Pointer capture can retarget the synthesized click to the canvas.
        let target = document().element_from_point(ev.client_x() as f32, ev.client_y() as f32).map(Into::into);
        if closest(target.clone(), "[data-node]").is_some() { return; }
        let point = closest(target.clone(), "[data-wire-point]");
        let Some(el) = point.clone().or_else(|| closest(target, "[data-wire]")) else { return };
        let Some(key) = parse_wire(&el).as_ref().and_then(selection_key) else { return };
        let Some(canvas) = self.canvas.get_untracked() else { return };
        let rect = canvas.get_bounding_client_rect();
        let world = self.ed.view.get_untracked().to_world(ev.client_x() as f64 - rect.left(), ev.client_y() as f64 - rect.top());
        let edit = match point.and_then(|p| p.get_attribute("data-wire-point")?.parse().ok()) {
            Some(index) => PointEdit::Remove(index), None => PointEdit::Add,
        };
        ev.prevent_default();
        edit_route(self.ed, key, world, edit);
    }

    pub(super) fn send_point(self, key: WireKey, origin: Vec<Position>) {
        if !self.ed.wire_exists_untracked(key) { return; }
        let points = self.ed.wire_routes.with_untracked(|all| all.get(&key).cloned().unwrap_or_default());
        if points != origin {
            self.ed.send(ClientMessage::SetWireRoute(WireRoute { exec: key.0, connection: key.1, points }));
        }
    }
}
