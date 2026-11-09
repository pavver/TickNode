//! Updating and finishing pointer gestures, including pan and zoom.

use super::*;

impl Interaction {
    pub(in crate::canvas) fn finish(self, ev: &PointerEvent, commit: bool) {
        let Self { ed, drag, panning, pending_wire, select_box, pointers, .. } = self;
        pointers.update_value(|p| {
            p.remove(&ev.pointer_id());
        });
        let state = drag.get_value();
        drag.set_value(Drag::Idle);
        panning.set(false);
        pending_wire.set(None);
        let boxed = select_box.get_untracked();
        select_box.set(None);
        match (state, commit) {
            // Interrupted drags still synchronize positions with the server.
            (Drag::Nodes { origins, routes, moved: true, .. }, _) => self.send_moves(origins, routes),
            (Drag::Point { key, origin, moved: true, .. }, _) => self.send_point(key, origin),
            (_, false) => {},
            (Drag::Pan { moved: false, .. }, true) => ed.selection.set(None),
            // Clicking a node in a group selects only that node.
            (Drag::Nodes { clicked, .. }, true) => ed.select_nodes([clicked]),
            (Drag::Box { start, additive }, true) => self.finish_box(boxed.unwrap_or(SelectBox { start, end: start }), additive),
            (Drag::Wire { from }, true) => self.finish_wire(ev, from),
            _ => {},
        }
    }

    fn finish_box(self, b: SelectBox, additive: bool) {
        if distance(b.start, b.end) <= DRAG_THRESHOLD {
            if !additive { self.ed.selection.set(None); }
            return;
        }
        let mut ids = nodes_in_box(self.ed, b);
        if additive { ids.extend(self.ed.selected_nodes()); }
        self.ed.select_nodes(ids);
    }

    fn finish_wire(self, ev: &PointerEvent, from: WireEnd) {
        let hit = document().element_from_point(ev.client_x() as f32, ev.client_y() as f32);
        let Some(hit) = hit else { return };
        let to = hit.closest("[data-pin]").ok().flatten().and_then(|el| parse_pin(&el));
        let Some(to) = to else {
            // Pointer capture retargets pointerup to the canvas; use the element under the cursor.
            let background = hit.closest(".canvas").ok().flatten().is_some()
                && hit.closest("[data-node], [data-wire], button, input, select, textarea").ok().flatten().is_none();
            if background {
                let local = self.local_point(ev);
                let world = self.ed.view.get_untracked().to_world(local.0, local.1);
                self.ed.wire_context.set(None);
                self.ed.picker.set(None);
                self.ed.wire_menu.set(Some(crate::state::WireMenu {
                    from, position: Position { x: world.0 as f32, y: world.1 as f32 },
                    screen: (ev.client_x() as f64, ev.client_y() as f64),
                }));
            }
            return;
        };
        match decisions::wire_command(from, to) {
            Ok(Some(command)) => self.ed.send(command),
            Err(message) => self.ed.log(LogKind::Error, message),
            Ok(None) => {}
        }
    }

    pub(in crate::canvas) fn pointer_move(self, ev: PointerEvent) {
        let Self { ed, drag, cursor, select_box, pointers, .. } = self;
        let p = self.local_point(&ev);
        pointers.update_value(|ps| {
            if let Some(v) = ps.get_mut(&ev.pointer_id()) {
                *v = p;
            }
        });
        ed.pointer_world.set_value(Some(ed.view.get_untracked().to_world(p.0, p.1)));
        move_paste(ed, p);
        match drag.get_value() {
            Drag::Idle => {}
            Drag::Box { start, .. } => select_box.set(Some(SelectBox { start, end: p })),
            Drag::Pan { start, origin, moved } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                drag.set_value(Drag::Pan { start, origin, moved });
                ed.view.set(Camera { x: origin.x + p.0 - start.0, y: origin.y + p.1 - start.1, ..origin });
            }
            Drag::Nodes { origins, routes, start_world, start, moved, clicked } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                if moved { self.move_nodes(&origins, &routes, clicked, start_world, p); }
                drag.set_value(Drag::Nodes { origins, routes, start_world, start, moved, clicked });
            }
            Drag::Point { key, index, origin, start_world, start, moved } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                if moved && self.ed.wire_exists_untracked(key) {
                    let world = ed.view.get_untracked().to_world(p.0, p.1);
                    let mut points = origin.clone();
                    if let Some(point) = points.get_mut(index) {
                        *point = moved_position(*point, (world.0 - start_world.0, world.1 - start_world.1));
                        ed.wire_routes.update(|all| { all.insert(key, points); });
                    }
                }
                drag.set_value(Drag::Point { key, index, origin, start_world, start, moved });
            }
            Drag::Wire { .. } => cursor.set(ed.view.get_untracked().to_world(p.0, p.1)),
            Drag::Pinch { start_dist, start_mid, origin } => {
                self.move_pinch(start_dist, start_mid, origin);
            }
        }
    }

    fn move_nodes(self, origins: &[(NodeId, Position)], routes: &WireRoutes, clicked: NodeId, start_world: (f64, f64), p: (f64, f64)) {
        let (wx, wy) = self.ed.view.get_untracked().to_world(p.0, p.1);
        let delta = (wx - start_world.0, wy - start_world.1);
        // Snap the dragged card, then translate the entire group without changing its layout.
        let anchor = origins.iter().find(|(id, _)| *id == clicked).map(|(_, p)| *p).unwrap_or_default();
        let delta = decisions::group_delta(anchor, delta);
        let shifted = |origin: Position| Position { x: origin.x + delta.x, y: origin.y + delta.y };
        for (id, origin) in origins {
            if let Some(node) = self.ed.node(*id) { node.pos.set(shifted(*origin)); }
        }
        if !routes.is_empty() {
            self.ed.wire_routes.update(|all| {
                for (key, points) in routes {
                    if self.ed.wire_exists_untracked(*key) { all.insert(*key, points.iter().copied().map(shifted).collect()); }
                }
            });
        }
    }

    fn move_pinch(self, start_dist: f64, start_mid: (f64, f64), origin: Camera) {
        let active: Vec<(f64, f64)> = self.pointers.with_value(|ps| ps.values().copied().collect());
        if let [a, b] = active.as_slice() { self.ed.view.set(pinch_camera(start_dist, start_mid, origin, *a, *b)); }
    }

    pub(in crate::canvas) fn wheel(self, ev: web_sys::WheelEvent) {
        let Self { ed, canvas, .. } = self;
        ev.prevent_default();
        let Some(c) = canvas.get_untracked() else { return };
        let r = c.get_bounding_client_rect();
        let line = if ev.delta_mode() == 1 { 16.0 } else { 1.0 };
        let factor = (-ev.delta_y() * line * 0.0015).exp();
        let (sx, sy) = (ev.client_x() as f64 - r.left(), ev.client_y() as f64 - r.top());
        ed.view.update(|v| *v = v.zoom_at(sx, sy, factor));
    }
}

fn moved_position(origin: Position, delta: (f64, f64)) -> Position {
    let snap = |v: f64| ((v / DU).round() * DU) as f32;
    Position { x: snap(origin.x as f64 + delta.0), y: snap(origin.y as f64 + delta.1) }
}

fn pinch_camera(start_dist: f64, start_mid: (f64, f64), origin: Camera, a: (f64, f64), b: (f64, f64)) -> Camera {
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let zoom = (origin.zoom * distance(a, b) / start_dist).clamp(MIN_ZOOM, MAX_ZOOM);
    let (wx, wy) = origin.to_world(start_mid.0, start_mid.1);
    Camera { x: mid.0 - wx * zoom, y: mid.1 - wy * zoom, zoom }
}

#[cfg(test)]
mod tests;
