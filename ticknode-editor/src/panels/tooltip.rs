//! Node help button and its tooltip overlay.

use leptos::prelude::*;
use ticknode_shared::*;
use wasm_bindgen::JsCast;
use web_sys::PointerEvent;

use crate::geometry::DU;
use crate::i18n::{self, t};
use crate::state::{Tooltip, use_editor};
use crate::ui::{Icon, icons};

/// Width of the "info" tooltip, in px.
const TOOLTIP_WIDTH: f64 = 80.0 * DU;

/// "Info" button: shows a detailed node explanation on hover (or tap).
#[component]
pub(super) fn InfoButton(def: NodeDefinition) -> impl IntoView {
    let ed = use_editor();
    let show = move |ev: &web_sys::MouseEvent| {
        let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
        let r = el.get_bounding_client_rect();
        ed.tooltip.set(Some(Tooltip {
            title: i18n::node_title(&def),
            text: i18n::node_help(&def),
            anchor: (r.left(), r.top(), r.right(), r.bottom()),
        }));
    };
    let show_hover = show.clone();
    view! {
        <span
            class="info-btn"
            role="button"
            aria-label=move || t!("palette-info-aria")
            on:mouseenter=move |ev| show_hover(&ev)
            on:mouseleave=move |_| ed.tooltip.set(None)
            // Do not start dragging the palette block
            on:pointerdown=|ev: PointerEvent| ev.stop_propagation()
            // Touch devices have no hover: tap shows the tooltip, tapping elsewhere hides it (see App)
            on:click=move |ev: web_sys::MouseEvent| {
                ev.stop_propagation();
                show(&ev);
            }
        >
            <Icon path=icons::INFO />
        </span>
    }
}

/// "Info" tooltip overlay: positioned to the right of the button, or to the left if it doesn't fit.
#[component]
pub fn TooltipLayer() -> impl IntoView {
    let ed = use_editor();
    move || {
        let tip = ed.tooltip.get()?;
        let (left, top, right, bottom) = tip.anchor;
        let (vw, vh) = (
            window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0),
            window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0),
        );
        let width = TOOLTIP_WIDTH.min(vw - 4.0 * DU);
        let gap = 2.0 * DU;
        let x = if right + gap + width <= vw { right + gap } else { (left - gap - width).max(2.0 * DU) };
        // In the lower half of the screen, align with the button's bottom edge to keep the tooltip on-screen
        let y = if top < vh / 2.0 { format!("top:{top}px") } else { format!("bottom:{}px", vh - bottom) };
        Some(view! {
            <div class="tooltip" style=format!("left:{x}px;{y};width:{width}px")>
                <div class="tooltip-title">{tip.title}</div>
                <div class="tooltip-text">{tip.text}</div>
            </div>
        })
    }
}
