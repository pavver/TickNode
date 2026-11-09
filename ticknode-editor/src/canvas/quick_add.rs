//! Browser adapters for quick-add opening; graph creation goes through the server.

use leptos::prelude::*;
use ticknode_shared::Position;
use super::interaction::{Drag, Interaction};
use crate::geometry::DU;
use crate::quick_add::QuickAdd;

/// Shared gate for keyboard and background double-click openings.
pub(super) fn can_open(paste: bool, dragging: bool, popup: bool) -> bool {
    !paste && !dragging && !popup
}

impl Interaction {
    fn can_quick_add(self) -> bool {
        let ed = self.ed;
        if ed.open_sheet.get_untracked().is_none() {
            return false;
        }
        can_open(
            ed.pending_paste.with_untracked(|p| p.is_some()),
            self.drag.with_value(|d| !matches!(d, Drag::Idle))
                || ed.palette_drag.with_untracked(|d| d.is_some())
                || document().query_selector(".node-resize:active").ok().flatten().is_some()
                || ed.nodes.with_untracked(|nodes| nodes.values().any(|n| n.size_preview.with_untracked(|s| s.is_some()))),
            ed.picker.with_untracked(|p| p.is_some()) || ed.wire_menu.with_untracked(|p| p.is_some())
                || ed.resource_menu.with_untracked(|p| p.is_some())
                || ed.wire_context.with_untracked(|p| p.is_some()) || ed.quick_add.with_untracked(|p| p.is_some())
                || ed.tooltip.with_untracked(|p| p.is_some()),
        )
    }

    pub(super) fn quick_add_key(self, ev: web_sys::KeyboardEvent, over: bool, pointer: Option<(f64, f64)>) {
        if !matches!(ev.key().as_str(), " " | "Tab") || ev.ctrl_key() || ev.meta_key() || ev.alt_key()
            || ev.shift_key() || ev.repeat() || ev.default_prevented() || crate::typing_in_field(&ev)
            || !over || !self.can_quick_add() { return; }
        let Some(canvas) = self.canvas.get_untracked() else { return };
        let rect = canvas.get_bounding_client_rect();
        let screen = pointer.unwrap_or((rect.left() + rect.width() / 2.0, rect.top() + rect.height() / 2.0));
        if self.open_quick_add(screen) { ev.prevent_default(); }
    }

    pub(super) fn open_quick_add(self, screen: (f64, f64)) -> bool {
        if !self.can_quick_add() { return false; }
        let Some(canvas) = self.canvas.get_untracked() else { return false };
        let rect = canvas.get_bounding_client_rect();
        let world = self.ed.view.get_untracked().to_world(screen.0 - rect.left(), screen.1 - rect.top());
        let snap = |v: f64| ((v / DU).round() * DU) as f32;
        self.ed.quick_add.set(Some(QuickAdd { screen, position: Position { x: snap(world.0), y: snap(world.1) } }));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::can_open;

    #[test]
    fn paste_gestures_and_other_popups_block_opening() {
        assert!(can_open(false, false, false));
        for flags in [(true, false, false), (false, true, false), (false, false, true)] {
            assert!(!can_open(flags.0, flags.1, flags.2));
        }
    }
}
