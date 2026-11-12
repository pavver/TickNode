use super::*;

impl Interaction {
    pub(in crate::canvas) fn finish(self, ev: &PointerEvent, commit: bool) {
        let Self {
            ed,
            drag,
            panning,
            pending_wire,
            select_box,
            pointers,
            ..
        } = self;
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
            (
                Drag::Nodes {
                    origins,
                    routes,
                    moved: true,
                    ..
                },
                _,
            ) => self.send_moves(origins, routes),
            (
                Drag::Frame {
                    frame_id,
                    frame_origins,
                    node_origins,
                    routes,
                    moved: true,
                    ..
                },
                true,
            ) => {
                self.send_frame_moves(frame_id, frame_origins, node_origins, routes);
            }
            (
                Drag::Frame {
                    frame_id,
                    frame_origins,
                    node_origins,
                    routes,
                    moved: false,
                    ..
                },
                true,
            ) => {
                self.cancel_frame_drag(frame_origins, node_origins, routes);
                ed.selection.set(Some(Selection::Frame(frame_id)));
            }
            (
                Drag::Frame {
                    frame_origins,
                    node_origins,
                    routes,
                    ..
                },
                false,
            ) => {
                self.cancel_frame_drag(frame_origins, node_origins, routes);
            }
            (
                Drag::Point {
                    key,
                    before,
                    moved: true,
                    ..
                },
                _,
            ) => self.send_point(key, before),
            (_, false) => {}
            (Drag::Pan { moved: false, .. }, true) => ed.selection.set(None),
            // Clicking a node in a group selects only that node.
            (Drag::Nodes { clicked, .. }, true) => ed.select_nodes([clicked]),
            (Drag::Box { start, additive }, true) => {
                self.finish_box(boxed.unwrap_or(SelectBox { start, end: start }), additive)
            }
            (Drag::Wire { from, detached }, true) => self.finish_wire(ev, from, detached),
            _ => {}
        }
    }

    fn finish_box(self, b: SelectBox, additive: bool) {
        if distance(b.start, b.end) <= DRAG_THRESHOLD {
            if !additive {
                self.ed.selection.set(None);
            }
            return;
        }
        let mut ids = nodes_in_box(self.ed, b);
        if additive {
            ids.extend(self.ed.selected_nodes());
        }
        self.ed.select_nodes(ids);
    }

    fn finish_wire(self, ev: &PointerEvent, from: WireEnd, detached: Option<DetachedConnection>) {
        let hit = document().element_from_point(ev.client_x() as f32, ev.client_y() as f32);
        let Some(hit) = hit else { return };
        let to = hit
            .closest("[data-pin]")
            .ok()
            .flatten()
            .and_then(|el| parse_pin(&el));
        if let Some(d) = detached {
            if !self.ed.detached.with_untracked(|all| all.contains(&d)) {
                return;
            }
            if let Some(to) = to {
                let kind = |end: WireEnd| {
                    self.ed.node(end.node).and_then(|n| {
                        n.instance
                            .with_untracked(|i| pin_type(&i.definition, end.pin, end.dir).cloned())
                    })
                };
                let command = self.ed.types.with_untracked(|types| {
                    self.ed.data.with_untracked(|data| {
                        crate::canvas::detached::reconnect(
                            d,
                            to,
                            kind(from).as_ref(),
                            kind(to).as_ref(),
                            types,
                            data,
                        )
                    })
                });
                if let Some(command) = command {
                    self.ed.send(command);
                }
            }
            return;
        }
        let Some(to) = to else {
            // Pointer capture retargets pointerup to the canvas; use the element under the cursor.
            let background = hit.closest(".canvas").ok().flatten().is_some()
                && hit
                    .closest("[data-node], [data-wire], button, input, select, textarea")
                    .ok()
                    .flatten()
                    .is_none();
            if background {
                let local = self.local_point(ev);
                let world = self.ed.view.get_untracked().to_world(local.0, local.1);
                self.ed.wire_context.set(None);
                self.ed.picker.set(None);
                self.ed.wire_menu.set(Some(crate::state::WireMenu {
                    from,
                    position: Position {
                        x: world.0 as f32,
                        y: world.1 as f32,
                    },
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
}
