//! Fixed-position menu for route points and detached wire removal.

use leptos::prelude::*;
use wasm_bindgen::{JsCast, closure::Closure};
use ticknode_shared::ClientMessage;

use crate::canvas::{PointEdit, edit_route};
use crate::i18n::t;
use crate::picker::{placement, viewport};
use crate::state::{WireContext, use_editor};

#[component]
pub fn WireContextLayer() -> impl IntoView {
    let ed = use_editor();
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() { ed.wire_context.set(None); }
    });
    Effect::new(move |_| {
        if let Some(state) = ed.wire_context.get() {
            let exists = state.detached.map_or_else(|| ed.wire_exists((state.exec, state.connection)), |d| ed.detached.with(|all| all.contains(&d)));
            if !exists { ed.wire_context.set(None); }
        }
    });
    let outside = Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |ev: web_sys::PointerEvent| {
        let inside = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".wire-context").ok().flatten()).is_some();
        if !inside && ed.wire_context.get_untracked().is_some() { ed.wire_context.set(None); }
    });
    let _ = window().add_event_listener_with_callback_and_bool("pointerdown", outside.as_ref().unchecked_ref(), true);
    let outside = StoredValue::new_local(outside);
    on_cleanup(move || outside.with_value(|listener| {
        let _ = window().remove_event_listener_with_callback_and_bool("pointerdown", listener.as_ref().unchecked_ref(), true);
    }));
    move || ed.wire_context.get().map(|state| view! { <WireContextPopup state=state /> })
}

#[component]
fn WireContextPopup(state: WireContext) -> impl IntoView {
    let ed = use_editor();
    let button = NodeRef::<leptos::html::Button>::new();
    Effect::new(move |_| { if let Some(el) = button.get() { let _ = el.focus(); } });
    let key = (state.exec, state.connection);
    let has_points = move || state.detached.is_none() && ed.wire_routes.with(|all| all.get(&key).is_some_and(|points| !points.is_empty()));
    let choose = move |edit| {
        edit_route(ed, key, state.world, edit);
        ed.wire_context.set(None);
    };
    let (x, y) = state.screen;
    let place = placement((x, y, x, y), viewport());
    let vertical = if place.up { format!("bottom:{}px", place.edge) } else { format!("top:{}px", place.edge) };
    // A short action list sizes to its content instead of the picker width.
    let style = format!("left:{}px;width:max-content;{vertical};max-height:{}px", place.left, place.max_height);
    view! {
        <div class="picker-popup wire-context" style=style
            on:pointerdown=|ev| ev.stop_propagation() on:contextmenu=|ev| ev.prevent_default()
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" { ev.prevent_default(); ed.wire_context.set(None); }
            }>
            {if let Some(d) = state.detached {
                view! { <button node_ref=button class="picker-row" on:click=move |_| {
                    ed.send(ClientMessage::RemoveDetached(d));
                    ed.wire_context.set(None);
                }>{move || t!("wire-detached-remove")}</button> }.into_any()
            } else { match state.index {
                Some(index) => view! { <button node_ref=button class="picker-row" on:click=move |_| choose(PointEdit::Remove(index))>{move || t!("wire-point-remove")}</button> }.into_any(),
                None => view! { <button node_ref=button class="picker-row" on:click=move |_| choose(PointEdit::Add)>{move || t!("wire-point-add")}</button> }.into_any(),
            }}}
            <Show when=has_points>
                <button class="picker-row" on:click=move |_| choose(PointEdit::Clear)>{move || t!("wire-points-clear")}</button>
            </Show>
        </div>
    }
}
