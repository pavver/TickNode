//! Node copy and paste. A snippet is a `GraphModel` serialized as JSON with a format tag, so it can
//! be pasted into another editor (tab) or saved as text. Works via document `copy`/`paste` events —
//! without the Clipboard API, which requires HTTPS.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use ticknode_shared::*;

use crate::canvas::view_center;
use crate::geometry::DU;
use crate::state::{Editor, LogKind, PendingPaste};

const FORMAT: &str = "ticknode/snippet@1";

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    graph: GraphModel,
}

/// Selected nodes or frame with current positions and connections.
pub fn selection_snippet(ed: Editor) -> Option<GraphModel> {
    use crate::state::Selection;
    let (ids, frames) = match ed.selection.get_untracked() {
        Some(Selection::Nodes(ids)) => {
            if ids.is_empty() {
                return None;
            }
            (ids.into_iter().collect::<Vec<_>>(), Vec::new())
        }
        Some(Selection::Frame(id)) => {
            let frame = ed.frames.with_untracked(|all| all.get(&id).cloned())?;
            let rect = (
                frame.position.x as f64,
                frame.position.y as f64,
                frame.size.width,
                frame.size.height,
            );
            let nodes = ed.nodes.with_untracked(|nodes| {
                nodes
                    .iter()
                    .map(|(n_id, node)| {
                        let pos = node.pos.get_untracked();
                        let l = ed.layout_of(*node);
                        (*n_id, pos, l.width, l.height)
                    })
                    .collect::<Vec<_>>()
            });
            let inside_nodes = crate::canvas::nodes_fully_inside(rect, nodes);

            let all_frames = ed.frames.with_untracked(|all| {
                all.iter()
                    .map(|(f_id, f)| (*f_id, f.position, f.size))
                    .collect::<Vec<_>>()
            });
            let inside_frame_ids = crate::canvas::frames_fully_inside(rect, Some(id), all_frames);
            let mut frames = vec![frame];
            frames.extend(
                inside_frame_ids
                    .into_iter()
                    .filter_map(|f_id| ed.frames.with_untracked(|all| all.get(&f_id).cloned())),
            );
            (inside_nodes, frames)
        }
        _ => return None,
    };

    let nodes = ids
        .iter()
        .filter_map(|id| {
            let node = ed.node(*id)?;
            let mut model = node.instance.with_untracked(|i| i.model.clone());
            model.position = node.pos.get_untracked();
            Some(model)
        })
        .collect();
    let inside = |c: &ConnectionModel| ids.contains(&c.src_node) && ids.contains(&c.dst_node);
    let exec_connections: Vec<_> = ed
        .exec
        .with_untracked(|e| e.iter().copied().filter(inside).collect());
    let data_connections: Vec<_> = ed
        .data
        .with_untracked(|d| d.iter().copied().filter(inside).collect());
    let wire_routes = ed.wire_routes.with_untracked(|all| {
        all.iter()
            .filter(|((exec, c), _)| {
                if *exec {
                    exec_connections.contains(c)
                } else {
                    data_connections.contains(c)
                }
            })
            .map(|((exec, connection), points)| WireRoute {
                exec: *exec,
                connection: *connection,
                points: points.clone(),
            })
            .collect()
    });
    let detached_connections = ed.detached.with_untracked(|all| {
        all.iter()
            .copied()
            .filter(|d| inside(&d.connection))
            .collect()
    });
    Some(GraphModel {
        nodes,
        exec_connections,
        data_connections,
        wire_routes,
        detached_connections,
        frames,
    })
}

pub fn to_text(graph: GraphModel) -> String {
    serde_json::to_string(&Envelope {
        format: FORMAT.into(),
        graph,
    })
    .expect("the fragment serializes")
}

pub fn from_text(text: &str) -> Option<GraphModel> {
    let envelope: Envelope = serde_json::from_str(text.trim()).ok()?;
    (envelope.format == FORMAT
        && (!envelope.graph.nodes.is_empty() || !envelope.graph.frames.is_empty()))
    .then_some(envelope.graph)
}

/// Top-left corner of a snippet across its nodes and frames.
fn snippet_origin(graph: &GraphModel) -> Option<(f64, f64)> {
    let points = graph
        .nodes
        .iter()
        .map(|n| n.position)
        .chain(graph.frames.iter().map(|f| f.position));
    let (x, y) = points.fold((f64::INFINITY, f64::INFINITY), |(x, y), p| {
        (x.min(p.x as f64), y.min(p.y as f64))
    });
    (x.is_finite() && y.is_finite()).then_some((x, y))
}

fn snap(v: f64) -> f32 {
    ((v / DU).round() * DU) as f32
}

pub fn snap_point((x, y): (f64, f64)) -> (f64, f64) {
    (snap(x) as f64, snap(y) as f64)
}

/// Offsets the snippet so its top-left corner lands at `at` and sends it to the server.
pub fn paste_at(ed: Editor, mut graph: GraphModel, at: (f64, f64)) {
    let Some((min_x, min_y)) = snippet_origin(&graph) else {
        return;
    };

    for node in &mut graph.nodes {
        node.position = Position {
            x: snap(node.position.x as f64 - min_x + at.0),
            y: snap(node.position.y as f64 - min_y + at.1),
        };
    }
    for frame in &mut graph.frames {
        frame.position = Position {
            x: snap(frame.position.x as f64 - min_x + at.0),
            y: snap(frame.position.y as f64 - min_y + at.1),
        };
    }
    offset_routes(&mut graph, (at.0 - min_x, at.1 - min_y), true);
    ed.send(ClientMessage::Paste(graph));
}

/// Pastes text from the clipboard: nodes are not created yet, but appear under the cursor (or at the
/// viewport center) and follow it until confirmed (`place_paste`) or cancelled (`cancel_paste`).
pub fn paste_text(ed: Editor, text: &str) {
    let Some(mut graph) = from_text(text) else {
        ed.log(
            LogKind::Error,
            "[ERROR] Clipboard does not contain TickNode nodes",
        );
        return;
    };
    let Some((min_x, min_y)) = snippet_origin(&graph) else {
        return;
    };

    for node in &mut graph.nodes {
        node.position = Position {
            x: (node.position.x as f64 - min_x) as f32,
            y: (node.position.y as f64 - min_y) as f32,
        };
    }
    for frame in &mut graph.frames {
        frame.position = Position {
            x: (frame.position.x as f64 - min_x) as f32,
            y: (frame.position.y as f64 - min_y) as f32,
        };
    }
    offset_routes(&mut graph, (-min_x, -min_y), false);
    let at = snap_point(
        ed.pointer_world
            .get_value()
            .unwrap_or_else(|| view_center(ed)),
    );
    ed.pending_paste.set(Some(PendingPaste { graph, at }));
}

fn offset_routes(graph: &mut GraphModel, delta: (f64, f64), snapped: bool) {
    for route in &mut graph.wire_routes {
        for point in &mut route.points {
            let (x, y) = (point.x as f64 + delta.0, point.y as f64 + delta.1);
            *point = if snapped {
                Position {
                    x: snap(x),
                    y: snap(y),
                }
            } else {
                Position {
                    x: x as f32,
                    y: y as f32,
                }
            };
        }
    }
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

/// Duplicates selected nodes or frame shifted down and to the right.
pub fn duplicate(ed: Editor) {
    let Some(graph) = selection_snippet(ed) else {
        return;
    };
    let Some((min_x, min_y)) = snippet_origin(&graph) else {
        return;
    };
    paste_at(ed, graph, (min_x + 6.0 * DU, min_y + 6.0 * DU));
}

/// Deletes the cut fragment (a cut frame also removes its enclosed nodes in one Batch).
pub fn delete_cut(ed: Editor, graph: &GraphModel) {
    if !graph.frames.is_empty() {
        let mut batch = Vec::new();
        for f in &graph.frames {
            batch.push(ClientMessage::RemoveFrame(f.id));
        }
        for n in &graph.nodes {
            batch.push(ClientMessage::RemoveNode(n.id));
        }
        ed.send_batch(batch);
    } else {
        delete_selection(ed);
    }
}

/// Deletes the selected nodes or wire.
pub fn delete_selection(ed: Editor) {
    use crate::state::Selection;
    let selection = ed.selection.get_untracked();
    if let Some((key, index)) = ed.selected_point.get_untracked() {
        ed.selected_point.set(None);
        let on_wire = match &selection {
            Some(Selection::Exec(c)) => key == (true, *c),
            Some(Selection::Data(c)) => key == (false, *c),
            _ => false,
        };
        if on_wire {
            crate::canvas::edit_route(ed, key, (0.0, 0.0), crate::canvas::PointEdit::Remove(index));
            return;
        }
    }
    match selection {
        Some(Selection::Nodes(ids)) => {
            ed.send_batch(ids.into_iter().map(ClientMessage::RemoveNode).collect())
        }
        Some(Selection::Exec(c)) => ed.send(ClientMessage::DisconnectExec(c)),
        Some(Selection::Data(c)) => ed.send(ClientMessage::DisconnectData {
            dst_node: c.dst_node,
            dst_pin: c.dst_pin,
        }),
        Some(Selection::Detached(d)) => ed.send(ClientMessage::RemoveDetached(d)),
        Some(Selection::Frame(id)) => ed.send(ClientMessage::RemoveFrame(id)),
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

#[cfg(test)]
mod tests;
