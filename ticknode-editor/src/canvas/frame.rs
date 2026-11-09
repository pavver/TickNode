//! Canvas frames (comment boxes): rendering, selection, renaming, recoloring and creation.

use leptos::prelude::*;
use ticknode_shared::*;

use crate::geometry::DU;
use crate::i18n::t;
use crate::state::{use_editor, Editor, Selection};

mod resize;
pub(super) use resize::FrameResizeHandle;

pub const FRAME_HEADER_H: f64 = 6.0 * DU;
pub const DEFAULT_FRAME_W: f64 = 400.0;
pub const DEFAULT_FRAME_H: f64 = 240.0;
pub const FRAME_PAD: f64 = 6.0 * DU;

pub const ALL_FRAME_COLORS: [FrameColor; 8] = [
    FrameColor::Gray,
    FrameColor::Red,
    FrameColor::Orange,
    FrameColor::Yellow,
    FrameColor::Green,
    FrameColor::Teal,
    FrameColor::Blue,
    FrameColor::Purple,
];

pub fn frame_color_class(color: FrameColor) -> &'static str {
    match color {
        FrameColor::Gray => "frame-gray",
        FrameColor::Red => "frame-red",
        FrameColor::Orange => "frame-orange",
        FrameColor::Yellow => "frame-yellow",
        FrameColor::Green => "frame-green",
        FrameColor::Teal => "frame-teal",
        FrameColor::Blue => "frame-blue",
        FrameColor::Purple => "frame-purple",
    }
}

pub fn frame_color_title(color: FrameColor) -> String {
    match color {
        FrameColor::Gray => t!("frame-color-gray"),
        FrameColor::Red => t!("frame-color-red"),
        FrameColor::Orange => t!("frame-color-orange"),
        FrameColor::Yellow => t!("frame-color-yellow"),
        FrameColor::Green => t!("frame-color-green"),
        FrameColor::Teal => t!("frame-color-teal"),
        FrameColor::Blue => t!("frame-color-blue"),
        FrameColor::Purple => t!("frame-color-purple"),
    }
}

pub fn snap_du_f32(v: f64) -> f32 {
    ((v / DU).round() * DU) as f32
}

pub fn snap_du_f64(v: f64) -> f64 {
    (v / DU).round() * DU
}

/// Returns whether the target rectangle (x, y, w, h) lies fully inside the container rectangle (cx, cy, cw, ch).
pub fn rect_contains(container: (f64, f64, f64, f64), target: (f64, f64, f64, f64)) -> bool {
    let (cx, cy, cw, ch) = container;
    let (tx, ty, tw, th) = target;
    const EPS: f64 = 1e-6;
    tx >= cx - EPS && (tx + tw) <= (cx + cw) + EPS && ty >= cy - EPS && (ty + th) <= (cy + ch) + EPS
}

/// Nodes whose rectangle lies fully inside the container rectangle.
pub fn nodes_fully_inside(
    container: (f64, f64, f64, f64),
    nodes: impl IntoIterator<Item = (NodeId, Position, f64, f64)>,
) -> Vec<NodeId> {
    nodes
        .into_iter()
        .filter(|(_, pos, w, h)| rect_contains(container, (pos.x as f64, pos.y as f64, *w, *h)))
        .map(|(id, ..)| id)
        .collect()
}

/// Frames whose rectangle lies fully inside the container rectangle (excluding an optional id).
pub fn frames_fully_inside(
    container: (f64, f64, f64, f64),
    exclude_id: Option<FrameId>,
    frames: impl IntoIterator<Item = (FrameId, Position, NodeSize)>,
) -> Vec<FrameId> {
    frames
        .into_iter()
        .filter(|(id, pos, size)| {
            if Some(*id) == exclude_id {
                return false;
            }
            rect_contains(container, (pos.x as f64, pos.y as f64, size.width, size.height))
        })
        .map(|(id, ..)| id)
        .collect()
}

/// Clamps and snaps a frame resize delta to valid bounds.
pub fn resize_frame_size(origin: NodeSize, delta: (f64, f64)) -> NodeSize {
    let snap = |v: f64| (v / DU).round() * DU;
    NodeSize {
        width: snap(origin.width + delta.0).clamp(MIN_FRAME_W, MAX_FRAME_SIZE),
        height: snap(origin.height + delta.1).clamp(MIN_FRAME_H, MAX_FRAME_SIZE),
    }
}

/// Builds the batch command of moved frames, nodes, and wire routes.
pub fn build_frame_move_batch(
    frames: impl IntoIterator<Item = Frame>,
    nodes: impl IntoIterator<Item = (NodeId, Position)>,
    routes: impl IntoIterator<Item = (crate::state::WireKey, Vec<Position>)>,
    delta: Position,
) -> Vec<ClientMessage> {
    if delta.x == 0.0 && delta.y == 0.0 {
        return Vec::new();
    }
    let mut commands = Vec::new();
    for mut frame in frames {
        frame.position.x += delta.x;
        frame.position.y += delta.y;
        commands.push(ClientMessage::SetFrame(frame));
    }
    for (id, pos) in nodes {
        commands.push(ClientMessage::MoveNode {
            id,
            position: Position {
                x: pos.x + delta.x,
                y: pos.y + delta.y,
            },
        });
    }
    for (key, points) in routes {
        let shifted = points
            .into_iter()
            .map(|p| Position {
                x: p.x + delta.x,
                y: p.y + delta.y,
            })
            .collect();
        commands.push(ClientMessage::SetWireRoute(WireRoute {
            exec: key.0,
            connection: key.1,
            points: shifted,
        }));
    }
    commands
}

pub fn calculate_new_frame_geometry(
    selected_node_bounds: Option<(f64, f64, f64, f64)>,
    view_center: (f64, f64),
) -> (Position, NodeSize) {
    if let Some((min_x, min_y, max_x, max_y)) = selected_node_bounds {
        let x0 = min_x - FRAME_PAD;
        let y0 = min_y - (FRAME_PAD + FRAME_HEADER_H);
        let w = (max_x - min_x) + 2.0 * FRAME_PAD;
        let h = (max_y - min_y) + 2.0 * FRAME_PAD + FRAME_HEADER_H;
        let width = snap_du_f64(w).max(MIN_FRAME_W);
        let height = snap_du_f64(h).max(MIN_FRAME_H);
        (Position { x: snap_du_f32(x0), y: snap_du_f32(y0) }, NodeSize { width, height })
    } else {
        let x0 = view_center.0 - DEFAULT_FRAME_W / 2.0;
        let y0 = view_center.1 - DEFAULT_FRAME_H / 2.0;
        (
            Position { x: snap_du_f32(x0), y: snap_du_f32(y0) },
            NodeSize { width: snap_du_f64(DEFAULT_FRAME_W), height: snap_du_f64(DEFAULT_FRAME_H) },
        )
    }
}

pub fn create_frame(ed: Editor) {
    let selected_ids = ed.selected_nodes();
    let bounds = if !selected_ids.is_empty() {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut count = 0;
        for id in selected_ids {
            let Some(node) = ed.node(id) else { continue };
            let pos = node.pos.get_untracked();
            let Some(layout) = ed.layout(id) else { continue };
            min_x = min_x.min(pos.x as f64);
            min_y = min_y.min(pos.y as f64);
            max_x = max_x.max(pos.x as f64 + layout.width);
            max_y = max_y.max(pos.y as f64 + layout.height);
            count += 1;
        }
        if count > 0 {
            Some((min_x, min_y, max_x, max_y))
        } else {
            None
        }
    } else {
        None
    };

    let center = crate::canvas::view_center(ed);
    let (position, size) = calculate_new_frame_geometry(bounds, center);
    let id = FrameId::generate();
    let frame = Frame {
        id,
        title: t!("frame-default-title"),
        color: FrameColor::Gray,
        position,
        size,
    };
    ed.pending_frame_select.set_value(Some(id));
    ed.send(ClientMessage::SetFrame(frame));
}

#[component]
pub fn FrameView(id: FrameId) -> impl IntoView {
    let ed = use_editor();
    let frame = Memo::new(move |_| ed.frames.with(|all| all.get(&id).cloned()));
    let is_selected = Memo::new(move |_| {
        ed.selection.with(|s| matches!(s, Some(Selection::Frame(sel_id)) if *sel_id == id))
    });

    let editing = RwSignal::new(false);
    let draft_title = RwSignal::new(String::new());
    let show_color_popup = RwSignal::new(false);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let commit_title = {
        let ed = ed;
        let id = id;
        move || {
            if !editing.get_untracked() {
                return;
            }
            editing.set(false);
            let trimmed = draft_title.get_untracked().trim().to_string();
            if let Some(mut f) = ed.frames.with_untracked(|all| all.get(&id).cloned()) {
                if !trimmed.is_empty() && trimmed != f.title {
                    f.title = trimmed;
                    ed.send(ClientMessage::SetFrame(f));
                }
            }
        }
    };

    let start_rename = move |current: String| {
        draft_title.set(current);
        editing.set(true);
        request_animation_frame(move || {
            if let Some(input) = input_ref.get() {
                let _ = input.focus();
                input.select();
            }
        });
    };

    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        match ev.key().as_str() {
            "Enter" => {
                ev.prevent_default();
                ev.stop_propagation();
                commit_title();
            }
            "Escape" => {
                ev.prevent_default();
                ev.stop_propagation();
                editing.set(false);
            }
            _ => {
                ev.stop_propagation();
            }
        }
    };

    let class = move || {
        let Some(f) = frame.get() else { return String::new() };
        let color_cls = frame_color_class(f.color);
        let sel = if is_selected.get() { " selected" } else { "" };
        format!("canvas-frame {color_cls}{sel}")
    };

    let style = move || {
        let Some(f) = frame.get() else { return String::new() };
        let pos = ed.frame_pos_previews.with(|p| p.get(&id).copied()).unwrap_or(f.position);
        let size = ed.frame_size_previews.with(|p| p.get(&id).copied()).unwrap_or(f.size);
        format!(
            "left:{}px;top:{}px;width:{}px;height:{}px;",
            pos.x, pos.y, size.width, size.height
        )
    };

    view! {
        <Show when=move || frame.get().is_some()>
            <div class=class style=style data-frame=id.to_string()>
                <div
                    class="canvas-frame-header"
                    on:pointerdown=crate::match_editor::stop_on_controls
                >
                    {move || if editing.get() {
                        view! {
                            <input
                                node_ref=input_ref
                                type="text"
                                class="inline-value canvas-frame-title-input"
                                maxlength={MAX_FRAME_TITLE.to_string()}
                                prop:value=move || draft_title.get()
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:input=move |ev| draft_title.set(event_target_value(&ev))
                                on:blur=move |_| commit_title()
                                on:keydown=on_keydown
                            />
                        }.into_any()
                    } else {
                        let cur_title = frame.get().map(|f| f.title).unwrap_or_default();
                        let title_for_click = cur_title.clone();
                        view! {
                            <span
                                class="canvas-frame-title ellipsis"
                                on:dblclick=move |ev| {
                                    ev.stop_propagation();
                                    start_rename(title_for_click.clone());
                                }
                            >
                                {cur_title}
                            </span>
                        }.into_any()
                    }}
                    <div style="position:relative;display:flex;align-items:center;">
                        <button
                            id=format!("frame-color-btn-{id}")
                            class="canvas-frame-color-btn"
                            title=move || frame.get().map(|f| frame_color_title(f.color)).unwrap_or_default()
                            aria-label=move || frame.get().map(|f| frame_color_title(f.color)).unwrap_or_default()
                            on:pointerdown=|ev| ev.stop_propagation()
                            on:click=move |ev| {
                                ev.stop_propagation();
                                show_color_popup.update(|open| *open = !*open);
                            }
                        />
                        <Show when=move || show_color_popup.get()>
                            <ColorPickerPopup
                                id=id
                                on_close=Callback::new(move |_| show_color_popup.set(false))
                            />
                        </Show>
                    </div>
                </div>
                <FrameResizeHandle id=id />
            </div>
        </Show>
    }
}

#[component]
fn ColorPickerPopup(id: FrameId, on_close: Callback<()>) -> impl IntoView {
    let ed = use_editor();

    let key_listener = window_event_listener(leptos::ev::keydown, move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Escape" {
            ev.prevent_default();
            ev.stop_propagation();
            on_close.run(());
        }
    });
    on_cleanup(move || key_listener.remove());

    let outside_listener = window_event_listener(leptos::ev::pointerdown, move |ev: web_sys::PointerEvent| {
        use wasm_bindgen::JsCast;
        let inside = ev.target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(&format!("#frame-color-popup-{id}, #frame-color-btn-{id}")).ok().flatten())
            .is_some();
        if !inside {
            on_close.run(());
        }
    });
    on_cleanup(move || outside_listener.remove());

    let choose = move |color: FrameColor| {
        if let Some(mut f) = ed.frames.with_untracked(|all| all.get(&id).cloned()) {
            if f.color != color {
                f.color = color;
                ed.send(ClientMessage::SetFrame(f));
            }
        }
        on_close.run(());
    };

    view! {
        <div
            id=format!("frame-color-popup-{id}")
            class="canvas-frame-color-popup"
            on:pointerdown=|ev| ev.stop_propagation()
        >
            {ALL_FRAME_COLORS.into_iter().map(|color| {
                let cls = format!("canvas-frame-color-swatch {}", frame_color_class(color));
                let title = frame_color_title(color);
                view! {
                    <button
                        class=cls
                        title=title.clone()
                        aria-label=title
                        on:click=move |ev| {
                            ev.stop_propagation();
                            choose(color);
                        }
                    />
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

#[cfg(test)]
mod tests;
