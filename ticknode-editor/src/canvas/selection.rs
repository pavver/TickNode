//! Selection marquee hit testing and selected node bounds.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::*;
use crate::state::{Editor, Selection, use_editor};

/// CAD-style selection marquee: left-to-right is a "window" (node must be completely inside),
/// right-to-left is "crossing" (touching is enough).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SelectBox {
    pub(super) start: (f64, f64),
    pub(super) end: (f64, f64),
}

impl SelectBox {
    pub(super) fn crossing(&self) -> bool {
        self.end.0 < self.start.0
    }

    pub(super) fn rect(&self) -> (f64, f64, f64, f64) {
        (self.start.0.min(self.end.0), self.start.1.min(self.end.1), self.start.0.max(self.end.0), self.start.1.max(self.end.1))
    }

    fn hits(&self, view: Camera, position: Position, (width, height): (f64, f64)) -> bool {
        let (x0, y0, x1, y1) = self.rect();
        let (wx0, wy0) = view.to_world(x0, y0);
        let (wx1, wy1) = view.to_world(x1, y1);
        let (nx0, ny0) = (position.x as f64, position.y as f64);
        let (nx1, ny1) = (nx0 + width, ny0 + height);
        if self.crossing() {
            nx0 <= wx1 && nx1 >= wx0 && ny0 <= wy1 && ny1 >= wy0
        } else {
            nx0 >= wx0 && nx1 <= wx1 && ny0 >= wy0 && ny1 <= wy1
        }
    }

}

/// Nodes captured by the selection box (in world coordinates).
pub(super) fn nodes_in_box(ed: Editor, b: SelectBox) -> Vec<NodeId> {
    let view = ed.view.get_untracked();
    ed.nodes.with_untracked(|nodes| {
        nodes.iter().filter(|(_, node)| {
            b.hits(view, node.pos.get_untracked(), untrack(|| { let l = ed.layout_of(**node); (l.width, l.height) }))
        }).map(|(id, _)| *id).collect()
    })
}

/// Padding of the selection bounds around nodes (in world coordinates). The group frame is wider
/// so it doesn't merge with individual node outlines (`.multi-select .node.selected::before`).
const BOUNDS_PAD: f64 = 3.0 * DU;
const GROUP_BOUNDS_PAD: f64 = 5.0 * DU;

/// Dashed bounding box around selected nodes; shows the node count for multi-selection.
#[component]
pub(super) fn SelectionBounds() -> impl IntoView {
    let ed = use_editor();
    let bounds = move || {
        let ids = ed.selection.with(|s| match s {
            Some(Selection::Nodes(set)) => set.iter().copied().collect::<Vec<_>>(),
            _ => Vec::new(),
        });
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut count = 0;
        for id in ids {
            let Some(node) = ed.node_tracked(id) else { continue };
            let p = node.pos.get();
            let Some(layout) = ed.layout(id) else { continue };
            let (w, h) = (layout.width, layout.height);
            x0 = x0.min(p.x as f64);
            y0 = y0.min(p.y as f64);
            x1 = x1.max(p.x as f64 + w);
            y1 = y1.max(p.y as f64 + h);
            count += 1;
        }
        let pad = if count > 1 { GROUP_BOUNDS_PAD } else { BOUNDS_PAD };
        (count > 0).then_some((x0 - pad, y0 - pad, x1 + pad, y1 + pad, count))
    };
    move || bounds().map(|(x0, y0, x1, y1, count)| view! {
        <div
            class="selection-bounds"
            style=format!("left:{x0}px;top:{y0}px;width:{}px;height:{}px", x1 - x0, y1 - y0)
        >
            {(count > 1).then(|| view! { <span class="selection-count">{count}</span> })}
        </div>
    })
}

#[cfg(test)]
mod tests;
