//! Start a wire or node drag after resolving the pointer hit.

use super::*;
use super::decisions::{NodeClick, node_click_action};
use crate::i18n::{self, t};
use crate::state::CopiedSettings;

impl Interaction {
    pub(super) fn start_node_click(self, id: NodeId, ev: &PointerEvent, start: (f64, f64), world: (f64, f64)) {
        let ed = self.ed;
        let Some(node) = ed.node(id) else { return };
        let instance = node.instance.get_untracked();
        let copied = ed.copied_settings.get_untracked();
        let action = node_click_action(
            ev.button(), ev.shift_key(), ev.ctrl_key() || ev.meta_key(),
            copied.as_ref().map(|s| s.type_name.as_str()), &instance.model.type_name,
        );
        match action {
            NodeClick::CopySettings => {
                ev.prevent_default();
                let title = i18n::node_title(&instance.definition);
                ed.log(LogKind::System, t!("settings-copied", title = title.clone()));
                ed.copied_settings.set(Some(CopiedSettings {
                    type_name: instance.model.type_name, title, properties: instance.model.properties,
                }));
            }
            NodeClick::PasteSettings => {
                if let Some(settings) = copied {
                    ed.send(ClientMessage::PasteSettings { id, type_name: settings.type_name, properties: settings.properties });
                }
            }
            NodeClick::ToggleSelect => self.toggle_node(id),
            NodeClick::Select => self.start_nodes(id, start, world),
            NodeClick::Ignore => {}
        }
    }

    pub(super) fn start_wire(self, end: WireEnd, world: (f64, f64)) {
        let mut from = end;
        // A connected data input relocates the existing wire.
        if end.dir == PinDirection::Input && !end.exec {
            let existing = self.ed.data.with_untracked(|d| d.iter().find(|c| c.dst_node == end.node && c.dst_pin == end.pin).copied());
            if let Some(c) = existing {
                self.ed.send(ClientMessage::DisconnectData { dst_node: c.dst_node, dst_pin: c.dst_pin });
                from = WireEnd { node: c.src_node, pin: c.src_pin, dir: PinDirection::Output, exec: false };
            }
        }
        self.cursor.set(world);
        self.pending_wire.set(Some(from));
        self.drag.set_value(Drag::Wire { from, detached: None });
    }

    pub(super) fn toggle_node(self, id: NodeId) {
        let mut ids = self.ed.selected_nodes();
        match ids.iter().position(|n| *n == id) {
            Some(i) => { ids.remove(i); }
            None => ids.push(id),
        }
        self.ed.select_nodes(ids);
    }

    pub(super) fn start_nodes(self, id: NodeId, start: (f64, f64), start_world: (f64, f64)) {
        let ed = self.ed;
        if !ed.selection.with_untracked(|s| matches!(s, Some(Selection::Nodes(set)) if set.contains(&id))) {
            ed.select_nodes([id]);
        }
        let origins: Vec<_> = ed.selected_nodes().into_iter()
            .filter_map(|n| ed.node(n).map(|node| (n, node.pos.get_untracked())))
            .collect();
        let routes = ed.wire_routes.with_untracked(|all| decisions::group_routes(all, &origins));
        self.drag.set_value(Drag::Nodes { origins, routes, start_world, start, moved: false, clicked: id });
    }

    pub(super) fn start_frame_drag(self, id: FrameId, start: (f64, f64), start_world: (f64, f64)) {
        let ed = self.ed;
        let Some(frame) = ed.frames.with_untracked(|all| all.get(&id).cloned()) else { return };
        let rect = (frame.position.x as f64, frame.position.y as f64, frame.size.width, frame.size.height);

        let nodes = ed.nodes.with_untracked(|nodes| {
            nodes.iter().map(|(n_id, node)| {
                let pos = node.pos.get_untracked();
                let layout = ed.layout_of(*node);
                (*n_id, pos, layout.width, layout.height)
            }).collect::<Vec<_>>()
        });
        let node_ids = crate::canvas::nodes_fully_inside(rect, nodes);
        let node_origins: Vec<(NodeId, Position)> = node_ids.into_iter().filter_map(|n_id| {
            ed.node(n_id).map(|node| (n_id, node.pos.get_untracked()))
        }).collect();

        let frames = ed.frames.with_untracked(|all| {
            all.iter().map(|(f_id, f)| (*f_id, f.position, f.size)).collect::<Vec<_>>()
        });
        let inside_frame_ids = crate::canvas::frames_fully_inside(rect, Some(id), frames);
        let mut frame_origins = vec![(frame.id, frame.position)];
        frame_origins.extend(inside_frame_ids.into_iter().filter_map(|f_id| {
            ed.frames.with_untracked(|all| all.get(&f_id).map(|f| (f_id, f.position)))
        }));

        let routes = ed.wire_routes.with_untracked(|all| decisions::group_routes(all, &node_origins));

        self.drag.set_value(Drag::Frame {
            frame_id: id,
            frame_origins,
            node_origins,
            routes,
            start_world,
            start,
            moved: false,
        });
    }
}
