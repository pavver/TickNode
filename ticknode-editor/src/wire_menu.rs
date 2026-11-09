//! Fixed-position node suggestions for a connection dropped on empty canvas.

use leptos::prelude::*;
use ticknode_shared::{ClientMessage, ConnectionEnd, PropertyOption};
use wasm_bindgen::{JsCast, closure::Closure};

use crate::i18n::{self, t};
use crate::picker::{KeyAction, MAX_ROWS, filter_options, key_action, placement, viewport};
use crate::state::{WireMenu, use_editor};
use crate::wire_suggest::suggestions;

#[component]
pub fn WireMenuLayer() -> impl IntoView {
    let ed = use_editor();
    Effect::new(move |first: Option<()>| {
        ed.view.track();
        if first.is_some() { ed.wire_menu.set(None); }
    });
    // Capture also sees presses on node controls that stop propagation to the canvas/window.
    let outside = Closure::<dyn Fn(web_sys::PointerEvent)>::new(move |ev: web_sys::PointerEvent| {
        let inside = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|el| el.closest(".wire-menu").ok().flatten()).is_some();
        if !inside && ed.wire_menu.get_untracked().is_some() { ed.wire_menu.set(None); }
    });
    let _ = window().add_event_listener_with_callback_and_bool("pointerdown", outside.as_ref().unchecked_ref(), true);
    let outside = StoredValue::new_local(outside);
    on_cleanup(move || {
        outside.with_value(|listener| {
            let _ = window().remove_event_listener_with_callback_and_bool("pointerdown", listener.as_ref().unchecked_ref(), true);
        });
    });
    move || ed.wire_menu.get().map(|state| view! { <WireMenuPopup state=state /> })
}

#[component]
fn WireMenuPopup(state: WireMenu) -> impl IntoView {
    let ed = use_editor();
    let query = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(el) = input.get() { let _ = el.focus(); }
    });
    let origin = Memo::new(move |_| {
        ed.node_tracked(state.from.node).and_then(|n| n.instance.with(|i| {
            if i.unavailable.is_some() { return None; }
            i.definition.pins.iter().find(|p| p.id == state.from.pin && p.direction == state.from.dir)
                .map(|p| p.pin_type.clone())
        }))
    });
    Effect::new(move |_| {
        if origin.get().is_none() { ed.wire_menu.set(None); }
    });
    let choices = Memo::new(move |_| {
        ed.locale.track();
        let Some(kind) = origin.get() else { return Vec::new() };
        ed.definitions.with(|catalog| ed.types.with(|types|
            suggestions(state.from.dir, &kind, catalog, types,
                |d| (i18n::node_title(d), crate::categories::category_label(&d.category)))))
    });
    let found = Memo::new(move |_| {
        let options: Vec<_> = choices.get().into_iter().map(|s| PropertyOption {
            value: s.type_name, label: s.title, detail: s.category, icon: None,
        }).collect();
        query.with(|q| filter_options(&options, q, MAX_ROWS))
    });
    let pick = move |index: usize| {
        if let Some(chosen) = choices.with_untracked(|c| c.get(index).cloned()) {
            ed.send(ClientMessage::AddNodeConnected {
                type_name: chosen.type_name, position: state.position,
                from: ConnectionEnd { node: state.from.node, pin: state.from.pin }, pin: chosen.pin,
            });
            ed.wire_menu.set(None);
        }
    };
    let on_key = move |ev: web_sys::KeyboardEvent| {
        let count = found.with_untracked(|(shown, _)| shown.len());
        let action = key_action(&ev.key(), active.get_untracked(), count);
        if action != KeyAction::Ignore { ev.prevent_default(); ev.stop_propagation(); }
        match action {
            KeyAction::Move(next) => {
                active.set(next);
                if let Some(el) = document().get_element_by_id(&format!("wire-menu-opt-{next}")) {
                    let options = web_sys::ScrollIntoViewOptions::new();
                    options.set_block(web_sys::ScrollLogicalPosition::Nearest);
                    el.scroll_into_view_with_scroll_into_view_options(&options);
                }
            }
            KeyAction::Pick => {
                if let Some(index) = found.with_untracked(|(shown, _)| shown.get(active.get_untracked()).copied()) { pick(index); }
            }
            KeyAction::Close => ed.wire_menu.set(None),
            KeyAction::Handled | KeyAction::Ignore => {}
        }
    };
    let (x, y) = state.screen;
    let place = placement((x, y, x, y), viewport());
    let vertical = if place.up { format!("bottom:{}px", place.edge) } else { format!("top:{}px", place.edge) };
    let style = format!("left:{}px;width:{}px;{vertical};max-height:{}px", place.left, place.width, place.max_height);
    view! {
        <div class="picker-popup wire-menu" style=style on:pointerdown=|ev| ev.stop_propagation()>
            <input node_ref=input class="picker-search" type="text" spellcheck="false" autocomplete="off"
                placeholder=move || t!("picker-search-placeholder")
                prop:value=move || query.get()
                on:input=move |ev| { query.set(event_target_value(&ev)); active.set(0); }
                on:keydown=on_key
            />
            <div class="picker-list">
                {move || {
                    let choices = choices.get();
                    let (shown, _) = found.get();
                    shown.into_iter().enumerate().map(|(row, index)| {
                        let choice = &choices[index];
                        view! {
                            <div id=format!("wire-menu-opt-{row}")
                                class=move || if active.get() == row { "picker-row active" } else { "picker-row" }
                                on:mousemove=move |_| if active.get_untracked() != row { active.set(row); }
                                on:click=move |_| pick(index)>
                                <span class="picker-text">
                                    <span class="picker-row-label">{choice.title.clone()}</span>
                                    <span class="picker-row-detail">{choice.category.clone()}</span>
                                </span>
                            </div>
                        }
                    }).collect_view()
                }}
                {move || {
                    let (shown, total) = found.get();
                    if shown.is_empty() {
                        Some(view! { <div class="picker-note">{t!("wire-menu-empty")}</div> })
                    } else if total > shown.len() {
                        Some(view! { <div class="picker-note">{t!("picker-more", count = total - shown.len())}</div> })
                    } else { None }
                }}
            </div>
        </div>
    }
}
