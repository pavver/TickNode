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
        if let Drag::Nodes { origins, moved: true, .. } = state {
            self.send_moves(origins);
            return;
        }
        if !commit {
            return;
        }
        match state {
            Drag::Pan { moved: false, .. } => ed.selection.set(None),
            // Clicking a node in a group without dragging selects only that node.
            Drag::Nodes { moved: false, clicked, .. } => ed.select_nodes([clicked]),
            Drag::Box { start, additive } => {
                let b = boxed.unwrap_or(SelectBox { start, end: start });
                if distance(b.start, b.end) <= DRAG_THRESHOLD {
                    if !additive {
                        ed.selection.set(None);
                    }
                    return;
                }
                let mut ids = nodes_in_box(ed, b);
                if additive {
                    ids.extend(ed.selected_nodes());
                }
                ed.select_nodes(ids);
            }
            Drag::Wire { from } => {
                let hit = document().element_from_point(ev.client_x() as f32, ev.client_y() as f32);
                let Some(to) = hit.and_then(|el| el.closest("[data-pin]").ok().flatten()).and_then(|el| parse_pin(&el)) else {
                    return;
                };
                if to.node == from.node && to.pin == from.pin {
                    return;
                }
                if to.dir == from.dir {
                    ed.log(LogKind::Error, "[ERROR] Only an output can be connected to an input");
                    return;
                }
                if to.exec != from.exec {
                    ed.log(LogKind::Error, "[ERROR] An exec pin can only be connected to an exec pin");
                    return;
                }
                let (out, inp) = if from.dir == PinDirection::Output { (from, to) } else { (to, from) };
                let c = ConnectionModel { src_node: out.node, src_pin: out.pin, dst_node: inp.node, dst_pin: inp.pin };
                ed.send(if from.exec { ClientMessage::ConnectExec(c) } else { ClientMessage::ConnectData(c) });
            }
            _ => {}
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
            Drag::Nodes { origins, start_world, start, moved, clicked } => {
                let moved = moved || distance(start, p) > DRAG_THRESHOLD;
                if moved {
                    let (wx, wy) = ed.view.get_untracked().to_world(p.0, p.1);
                    let (dx, dy) = (wx - start_world.0, wy - start_world.1);
                    // Snap to 1 DU grid
                    let snap = |v: f64| ((v / DU).round() * DU) as f32;
                    for (id, origin) in &origins {
                        if let Some(node) = ed.node(*id) {
                            node.pos.set(Position { x: snap(origin.x as f64 + dx), y: snap(origin.y as f64 + dy) });
                        }
                    }
                }
                drag.set_value(Drag::Nodes { origins, start_world, start, moved, clicked });
            }
            Drag::Wire { .. } => cursor.set(ed.view.get_untracked().to_world(p.0, p.1)),
            Drag::Pinch { start_dist, start_mid, origin } => {
                let active: Vec<(f64, f64)> = pointers.with_value(|ps| ps.values().copied().collect());
                if active.len() == 2 {
                    let mid = ((active[0].0 + active[1].0) / 2.0, (active[0].1 + active[1].1) / 2.0);
                    let zoom = (origin.zoom * distance(active[0], active[1]) / start_dist).clamp(MIN_ZOOM, MAX_ZOOM);
                    let (wx, wy) = origin.to_world(start_mid.0, start_mid.1);
                    ed.view.set(Camera { x: mid.0 - wx * zoom, y: mid.1 - wy * zoom, zoom });
                }
            }
        }
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
