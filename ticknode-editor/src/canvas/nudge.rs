//! Moving the selected nodes or frame with the arrow keys, like shapes in a text editor: a press
//! moves by one grid cell, Alt by one design unit and Shift by five grid cells. A burst of presses
//! is previewed locally and sent as one `Batch` (one Undo step) after a short pause.

use std::time::Duration;

use leptos::prelude::*;
use ticknode_shared::{FrameId, NodeId, Position};

use super::{build_frame_move_batch, frame_contents, interaction::group_routes, GRID};
use crate::geometry::DU;
use crate::state::{Editor, Selection, WireRoutes};

/// Presses closer together than this form one move (one history entry).
const IDLE: Duration = Duration::from_millis(400);
const LARGE: f64 = 5.0 * GRID;

/// Origins of everything a burst moves and the translation accumulated so far.
#[derive(Debug, Clone, Default)]
struct Burst {
    frames: Vec<(FrameId, Position)>,
    nodes: Vec<(NodeId, Position)>,
    routes: WireRoutes,
    delta: Position,
}

/// Editor-owned state of keyboard moves; `generation` lets only the latest timer flush.
#[derive(Debug, Clone, Default)]
pub struct NudgeState {
    burst: Option<Burst>,
    generation: u64,
}

/// The translation of one arrow key press, or `None` for other keys.
pub fn nudge_step(key: &str, alt: bool, shift: bool) -> Option<Position> {
    let (x, y) = match key {
        "ArrowLeft" => (-1.0, 0.0),
        "ArrowRight" => (1.0, 0.0),
        "ArrowUp" => (0.0, -1.0),
        "ArrowDown" => (0.0, 1.0),
        _ => return None,
    };
    let size = if alt { DU } else if shift { LARGE } else { GRID };
    Some(Position { x: (x * size) as f32, y: (y * size) as f32 })
}

/// Moves the selection by `step`; returns false when nothing selected can move.
pub fn nudge(ed: Editor, step: Position) -> bool {
    let Some((frames, nodes)) = targets(ed) else { return false };
    let same = ed.nudge.with_value(|s| s.burst.as_ref().is_some_and(|b| same_targets(b, &frames, &nodes)));
    if !same {
        flush_nudge(ed);
        let routes = ed.wire_routes.with_untracked(|all| group_routes(all, &nodes));
        ed.nudge.update_value(|s| s.burst = Some(Burst { frames, nodes, routes, delta: Position::default() }));
    }
    let Some(burst) = ed.nudge.try_update_value(|s| {
        let burst = s.burst.as_mut()?;
        burst.delta = shift(burst.delta, step);
        Some(burst.clone())
    }).flatten() else { return false };
    preview(ed, &burst);
    let generation = ed.nudge.try_update_value(|s| { s.generation += 1; s.generation }).unwrap_or_default();
    set_timeout(move || {
        if ed.nudge.try_with_value(|s| s.generation) == Some(generation) {
            flush_nudge(ed);
        }
    }, IDLE);
    true
}

/// Sends the pending burst, if any, as one history entry.
pub fn flush_nudge(ed: Editor) {
    let Some(burst) = ed.nudge.try_update_value(|s| s.burst.take()).flatten() else { return };
    let frames = burst.frames.iter().filter_map(|(id, origin)| {
        ed.frames.with_untracked(|all| all.get(id).cloned()).map(|mut f| { f.position = *origin; f })
    });
    ed.send_batch(build_frame_move_batch(frames, burst.nodes, burst.routes, burst.delta));
}

/// The selected frame with its contents, or the selected nodes; positions include unsent moves.
fn targets(ed: Editor) -> Option<(Vec<(FrameId, Position)>, Vec<(NodeId, Position)>)> {
    match ed.selection.get_untracked()? {
        Selection::Frame(id) => {
            let (frames, nodes) = frame_contents(ed, id)?;
            let frames = ed.frame_pos_previews.with_untracked(|previews| {
                frames.into_iter().map(|(f, pos)| (f, previews.get(&f).copied().unwrap_or(pos))).collect()
            });
            Some((frames, nodes))
        }
        Selection::Nodes(ids) => {
            let nodes: Vec<_> = ids.into_iter().filter_map(|id| ed.node(id).map(|n| (id, n.pos.get_untracked()))).collect();
            (!nodes.is_empty()).then_some((Vec::new(), nodes))
        }
        _ => None,
    }
}

fn same_targets(burst: &Burst, frames: &[(FrameId, Position)], nodes: &[(NodeId, Position)]) -> bool {
    burst.frames.iter().map(|(id, _)| id).eq(frames.iter().map(|(id, _)| id))
        && burst.nodes.iter().map(|(id, _)| id).eq(nodes.iter().map(|(id, _)| id))
}

fn shift(p: Position, by: Position) -> Position {
    Position { x: p.x + by.x, y: p.y + by.y }
}

/// Shows the burst at its current offset until the server confirms the move.
fn preview(ed: Editor, burst: &Burst) {
    for (id, origin) in &burst.nodes {
        if let Some(node) = ed.node(*id) {
            node.pos.set(shift(*origin, burst.delta));
        }
    }
    if !burst.frames.is_empty() {
        ed.frame_pos_previews.update(|previews| {
            for (id, origin) in &burst.frames {
                previews.insert(*id, shift(*origin, burst.delta));
            }
        });
    }
    if !burst.routes.is_empty() {
        ed.wire_routes.update(|all| {
            for (key, points) in &burst.routes {
                if all.contains_key(key) {
                    all.insert(*key, points.iter().map(|p| shift(*p, burst.delta)).collect());
                }
            }
        });
    }
}

#[cfg(test)]
mod tests;
