//! Node copy and paste. A snippet is a `GraphModel` serialized as JSON with a format tag, so it can
//! be pasted into another editor (tab) or saved as text. Works via document `copy`/`paste` events —
//! without the Clipboard API, which requires HTTPS.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use ticknode_shared::*;

use crate::canvas::view_center;
use crate::geometry::DU;
use crate::i18n::t;
use crate::state::{Editor, LogKind, PendingPaste};

const FORMAT: &str = "ticknode/snippet@1";

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    graph: GraphModel,
}

/// Selected nodes with their current positions and the connections between them.
pub fn selection_snippet(ed: Editor) -> Option<GraphModel> {
    let ids = ed.selected_nodes();
    if ids.is_empty() {
        return None;
    }
    let nodes = ids.iter().filter_map(|id| {
        let node = ed.node(*id)?;
        let mut model = node.instance.with_untracked(|i| i.model.clone());
        model.position = node.pos.get_untracked();
        Some(model)
    }).collect();
    let inside = |c: &ConnectionModel| ids.contains(&c.src_node) && ids.contains(&c.dst_node);
    Some(GraphModel {
        nodes,
        exec_connections: ed.exec.with_untracked(|e| e.iter().copied().filter(inside).collect()),
        data_connections: ed.data.with_untracked(|d| d.iter().copied().filter(inside).collect()),
    })
}

pub fn to_text(graph: GraphModel) -> String {
    serde_json::to_string(&Envelope { format: FORMAT.into(), graph }).expect("фрагмент серіалізується")
}

pub fn from_text(text: &str) -> Option<GraphModel> {
    let envelope: Envelope = serde_json::from_str(text.trim()).ok()?;
    (envelope.format == FORMAT && !envelope.graph.nodes.is_empty()).then_some(envelope.graph)
}

fn snap(v: f64) -> f32 {
    ((v / DU).round() * DU) as f32
}

pub fn snap_point((x, y): (f64, f64)) -> (f64, f64) {
    (snap(x) as f64, snap(y) as f64)
}

/// Offsets the snippet so its top-left corner lands at `at` and sends it to the server.
pub fn paste_at(ed: Editor, mut graph: GraphModel, at: (f64, f64)) {
    let min_x = graph.nodes.iter().map(|n| n.position.x as f64).fold(f64::INFINITY, f64::min);
    let min_y = graph.nodes.iter().map(|n| n.position.y as f64).fold(f64::INFINITY, f64::min);
    for node in &mut graph.nodes {
        node.position = Position { x: snap(node.position.x as f64 - min_x + at.0), y: snap(node.position.y as f64 - min_y + at.1) };
    }
    ed.send(ClientMessage::Paste(graph));
}

/// Pastes text from the clipboard: nodes are not created yet, but appear under the cursor (or at the
/// viewport center) and follow it until confirmed (`place_paste`) or cancelled (`cancel_paste`).
pub fn paste_text(ed: Editor, text: &str) {
    let Some(mut graph) = from_text(text) else {
        ed.log(LogKind::Error, t!("log-error-empty-clipboard"));
        return;
    };
    let min_x = graph.nodes.iter().map(|n| n.position.x).fold(f32::INFINITY, f32::min);
    let min_y = graph.nodes.iter().map(|n| n.position.y).fold(f32::INFINITY, f32::min);
    for node in &mut graph.nodes {
        node.position = Position { x: node.position.x - min_x, y: node.position.y - min_y };
    }
    let at = snap_point(ed.pointer_world.get_value().unwrap_or_else(|| view_center(ed)));
    ed.pending_paste.set(Some(PendingPaste { graph, at }));
}

pub fn place_paste(ed: Editor) {
    if let Some(p) = ed.pending_paste.get_untracked() {
        ed.pending_paste.set(None);
        paste_at(ed, p.graph, p.at);
    }
}

pub fn cancel_paste(ed: Editor) -> bool {
    let pending = ed.pending_paste.get_untracked().is_some();
    if pending {
        ed.pending_paste.set(None);
    }
    pending
}

/// Duplicates selected nodes shifted down and to the right.
pub fn duplicate(ed: Editor) {
    let Some(graph) = selection_snippet(ed) else { return };
    let min_x = graph.nodes.iter().map(|n| n.position.x as f64).fold(f64::INFINITY, f64::min);
    let min_y = graph.nodes.iter().map(|n| n.position.y as f64).fold(f64::INFINITY, f64::min);
    paste_at(ed, graph, (min_x + 6.0 * DU, min_y + 6.0 * DU));
}

/// Deletes the selected nodes or wire.
pub fn delete_selection(ed: Editor) {
    use crate::state::Selection;
    match ed.selection.get_untracked() {
        Some(Selection::Nodes(ids)) => ids.into_iter().for_each(|id| ed.send(ClientMessage::RemoveNode(id))),
        Some(Selection::Exec(c)) => ed.send(ClientMessage::DisconnectExec(c)),
        Some(Selection::Data(c)) => ed.send(ClientMessage::DisconnectData { dst_node: c.dst_node, dst_pin: c.dst_pin }),
        None => {}
    }
}

/// Copies the selection via a `copy` event (works from a button because it is triggered by a user gesture).
pub fn copy_via_command() -> bool {
    use wasm_bindgen::JsCast;
    leptos::prelude::document()
        .dyn_into::<web_sys::HtmlDocument>()
        .ok()
        .and_then(|d| d.exec_command("copy").ok())
        .unwrap_or(false)
}
