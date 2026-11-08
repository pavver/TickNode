//! Start a wire or node drag after resolving the pointer hit.

use super::*;

impl Interaction {
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
        self.drag.set_value(Drag::Wire { from });
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
        let origins = ed.selected_nodes().into_iter()
            .filter_map(|n| ed.node(n).map(|node| (n, node.pos.get_untracked())))
            .collect();
        self.drag.set_value(Drag::Nodes { origins, start_world, start, moved: false, clicked: id });
    }
}
