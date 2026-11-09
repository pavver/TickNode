//! Editor canvas: nodes (DOM) and wires (SVG) in a shared transformed layer.

use std::collections::HashMap;

use leptos::html;
use leptos::prelude::*;

use crate::geometry::DU;
use crate::i18n::t;
use crate::state::{Selection, use_editor};

mod camera;
mod quick_add;
mod detached;
mod interaction;
mod node;
mod paste;
mod pins;
mod selection;
mod wires;

pub use camera::{fit_view, view_center};
pub use node::trigger_pin;
pub use pins::*;

use interaction::{Drag, Interaction};
use node::NodeView;
use paste::PastePreview;
use selection::{SelectBox, SelectionBounds};
use wires::{PendingConnection, PendingWire, Wire};
pub(crate) use wires::WireEnd;
pub(crate) use interaction::{edit_route, PointEdit};

pub const CANVAS_ID: &str = "ticknode-canvas";
const GRID: f64 = 5.0 * DU;

#[component]
pub fn Canvas() -> impl IntoView {
    let ed = use_editor();
    let canvas = NodeRef::<html::Div>::new();
    let drag = StoredValue::new(Drag::Idle);
    let panning = RwSignal::new(false);
    let cursor = RwSignal::new((0.0, 0.0));
    let pending_wire = RwSignal::new(None::<WireEnd>);
    provide_context(PendingConnection(pending_wire));
    let select_box = RwSignal::new(None::<SelectBox>);
    let pointers = StoredValue::new(HashMap::<i32, (f64, f64)>::new());

    let interaction = Interaction { ed, canvas, drag, panning, cursor, pending_wire, select_box, pointers };
    let pointer = StoredValue::new(None::<(f64, f64)>);
    let over = StoredValue::new(false);
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        interaction.quick_add_key(ev, over.get_value(), pointer.get_value());
    });
    on_cleanup(move || keys.remove());

    let grid_style = move || {
        let v = ed.view.get();
        let size = GRID * v.zoom;
        format!("background-size:{size}px {size}px;background-position:{}px {}px", v.x, v.y)
    };
    let viewport_style = move || {
        let v = ed.view.get();
        format!("transform:translate({}px,{}px) scale({})", v.x, v.y, v.zoom)
    };
    let node_ids = move || ed.nodes.with(|n| n.keys().copied().collect::<Vec<_>>());

    view! {
        <div
            id=CANVAS_ID
            class="canvas"
            class:panning=move || panning.get()
            class:placing=move || ed.pending_paste.with(|p| p.is_some())
            node_ref=canvas
            style=grid_style
            on:pointerdown=move |ev| interaction.pointer_down(ev)
            on:dblclick=move |ev| interaction.double_click(ev)
            on:pointerenter=move |ev: web_sys::PointerEvent| {
                over.set_value(true);
                pointer.set_value(Some((ev.client_x() as f64, ev.client_y() as f64)));
            }
            on:pointermove=move |ev: web_sys::PointerEvent| {
                pointer.set_value(Some((ev.client_x() as f64, ev.client_y() as f64)));
                interaction.pointer_move(ev);
            }
            on:pointerup=move |ev| interaction.finish(&ev, true)
            on:pointercancel=move |ev| interaction.finish(&ev, false)
            on:pointerleave=move |_| { over.set_value(false); ed.pointer_world.set_value(None); }
            // Prevent the browser from activating middle-click autoscroll
            on:mousedown=|ev: web_sys::MouseEvent| if ev.button() == 1 { ev.prevent_default() }
            on:wheel=move |ev| interaction.wheel(ev)
            on:contextmenu=|ev| ev.prevent_default()
        >
            <div
                class="viewport"
                class:multi-select=move || ed.selection.with(|s| matches!(s, Some(Selection::Nodes(set)) if set.len() > 1))
                style=viewport_style
            >
                <svg class="wires">
                    <For each=move || ed.exec.get() key=|c| *c let:c>
                        <Wire conn=c exec=true />
                    </For>
                    <For each=move || ed.data.get() key=|c| *c let:c>
                        <Wire conn=c exec=false />
                    </For>
                    <For each=move || ed.detached.get() key=|d| *d let:d>
                        <detached::DetachedWire detached=d />
                    </For>
                    <PendingWire from=pending_wire cursor=cursor />
                </svg>
                <SelectionBounds />
                <For each=node_ids key=|id| *id let:id>
                    <NodeView id=id />
                </For>
                <PastePreview />
            </div>
            <Show when=move || ed.nodes.with(|n| n.is_empty())>
                <div class="canvas-hint">
                    <div class="t-section">{move || t!("canvas-empty-title")}</div>
                    <div class="t-caption">{move || t!("canvas-empty-hint")}</div>
                </div>
            </Show>
            {move || select_box.get().map(|b| {
                let (x0, y0, x1, y1) = b.rect();
                view! {
                    <div
                        class="select-box"
                        class:crossing=b.crossing()
                        style=format!("left:{x0}px;top:{y0}px;width:{}px;height:{}px", x1 - x0, y1 - y0)
                    ></div>
                }
            })}
            <div class="zoom-badge">{move || format!("{:.0}%", ed.view.get().zoom * 100.0)}</div>
        </div>
    }
}
