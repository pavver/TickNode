//! Canvas viewport lookup, centering and fitting.

use leptos::prelude::*;

use crate::geometry::*;
use crate::state::Editor;
use web_sys::Element;
use super::CANVAS_ID;

fn canvas_element() -> Option<Element> {
    document().get_element_by_id(CANVAS_ID)
}

/// Center of the visible viewport in world coordinates.
pub fn view_center(ed: Editor) -> (f64, f64) {
    let rect = canvas_element().map(|c| c.get_bounding_client_rect());
    let (w, h) = rect.map(|r| (r.width(), r.height())).unwrap_or((800.0, 600.0));
    ed.view.get_untracked().to_world(w / 2.0, h / 2.0)
}

/// Adjusts zoom and pan so that all nodes fit in the viewport.
pub fn fit_view(ed: Editor) {
    let Some(rect) = canvas_element().map(|c| c.get_bounding_client_rect()) else { return };
    let bounds = ed.nodes.with_untracked(|nodes| {
        nodes.values().fold(None, |acc: Option<(f64, f64, f64, f64)>, n| {
            let p = n.pos.get_untracked();
            let layout = untrack(|| ed.layout_of(*n));
            let (w, h) = (layout.width, layout.height);
            let (x0, y0, x1, y1) = (p.x as f64, p.y as f64, p.x as f64 + w, p.y as f64 + h);
            Some(match acc {
                None => (x0, y0, x1, y1),
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
            })
        })
    });
    let Some((x0, y0, x1, y1)) = bounds else {
        ed.view.set(Camera::default());
        return;
    };
    let pad = 10.0 * DU;
    let (bw, bh) = (x1 - x0 + 2.0 * pad, y1 - y0 + 2.0 * pad);
    let zoom = (rect.width() / bw).min(rect.height() / bh).clamp(MIN_ZOOM, 1.0);
    ed.view.set(Camera {
        x: (rect.width() - (x1 - x0) * zoom) / 2.0 - x0 * zoom,
        y: (rect.height() - (y1 - y0) * zoom) / 2.0 - y0 * zoom,
        zoom,
    });
}
